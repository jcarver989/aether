use std::mem::{Discriminant, discriminant, take};

use crate::components::thought_message::ThoughtMessage;
use crate::components::tool_call_statuses::ToolCallStatuses;
use tui::{Line, ViewContext, render_markdown};

#[derive(Debug, Clone)]
pub enum SegmentContent {
    Text(String),
    Thought(String),
    ToolCall(String),
}

#[derive(Debug)]
struct Segment {
    content: SegmentContent,
}

pub struct ConversationBuffer {
    segments: Vec<Segment>,
    thought_block_open: bool,
}

impl Default for ConversationBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationBuffer {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            thought_block_open: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn segments(&self) -> impl ExactSizeIterator<Item = &SegmentContent> {
        self.segments.iter().map(|s| &s.content)
    }

    pub fn append_text_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        self.close_thought_block();

        if let Some(segment) = self.segments.last_mut()
            && let SegmentContent::Text(existing) = &mut segment.content
        {
            existing.push_str(chunk);
        } else {
            self.segments.push(Segment {
                content: SegmentContent::Text(chunk.to_string()),
            });
        }
    }

    pub fn append_thought_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        if self.thought_block_open
            && let Some(segment) = self.segments.last_mut()
            && let SegmentContent::Thought(existing) = &mut segment.content
        {
            existing.push_str(chunk);
            return;
        }

        self.segments.push(Segment {
            content: SegmentContent::Thought(chunk.to_string()),
        });
        self.thought_block_open = true;
    }

    pub(crate) fn close_thought_block(&mut self) {
        self.thought_block_open = false;
    }

    pub(crate) fn clear(&mut self) {
        self.segments.clear();
        self.thought_block_open = false;
    }

    pub(crate) fn ensure_tool_segment(&mut self, tool_id: &str) {
        let has_segment = self
            .segments
            .iter()
            .any(|s| matches!(&s.content, SegmentContent::ToolCall(id) if id == tool_id));

        if !has_segment {
            self.segments.push(Segment {
                content: SegmentContent::ToolCall(tool_id.to_string()),
            });
        }
    }

    fn drain_segments_except(
        &mut self,
        mut keep: impl FnMut(&SegmentContent) -> bool,
    ) -> Vec<Segment> {
        let old = take(&mut self.segments);
        let (kept, removed) = old.into_iter().partition(|s| keep(&s.content));
        self.segments = kept;
        removed
    }

    pub(crate) fn drain_completed(
        &mut self,
        tool_call_statuses: &ToolCallStatuses,
    ) -> (Vec<SegmentContent>, Vec<String>) {
        let drained = self.drain_segments_except(|seg| {
            matches!(seg, SegmentContent::ToolCall(id) if tool_call_statuses.is_tool_running(id))
        });

        let mut content = Vec::new();
        let mut completed_tool_ids = Vec::new();

        for segment in drained {
            if let SegmentContent::ToolCall(ref id) = segment.content {
                completed_tool_ids.push(id.clone());
            }
            content.push(segment.content);
        }

        (content, completed_tool_ids)
    }
}

pub struct ConversationWindow<'a> {
    pub conversation: &'a ConversationBuffer,
    pub tool_call_statuses: &'a ToolCallStatuses,
}

impl ConversationWindow<'_> {
    pub fn render(&self, context: &ViewContext) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut last_segment_kind = None;

        for segment in &self.conversation.segments {
            let kind = discriminant(&segment.content);
            let rendered =
                render_stream_segment(&segment.content, self.tool_call_statuses, context);
            extend_with_vertical_margin(&mut lines, &mut last_segment_kind, kind, &rendered);
        }

        lines
    }
}

fn render_stream_segment(
    segment: &SegmentContent,
    tool_call_statuses: &ToolCallStatuses,
    context: &ViewContext,
) -> Vec<Line> {
    match segment {
        SegmentContent::Thought(text) => ThoughtMessage { text }.render(context),
        SegmentContent::Text(text) => render_markdown(text, context),
        SegmentContent::ToolCall(id) => tool_call_statuses.render_tool(id, context),
    }
}

pub fn render_segments_to_lines(
    segments: &[SegmentContent],
    tool_call_statuses: &ToolCallStatuses,
    context: &ViewContext,
) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut last_segment_kind = None;

    for segment in segments {
        let kind = discriminant(segment);
        let rendered = render_stream_segment(segment, tool_call_statuses, context);
        extend_with_vertical_margin(&mut lines, &mut last_segment_kind, kind, &rendered);
    }

    lines
}

fn extend_with_vertical_margin(
    target: &mut Vec<Line>,
    last_segment_kind: &mut Option<Discriminant<SegmentContent>>,
    kind: Discriminant<SegmentContent>,
    lines: &[Line],
) {
    if lines.is_empty() {
        return;
    }

    if let Some(prev_kind) = *last_segment_kind
        && prev_kind != kind
    {
        target.push(Line::new(String::new()));
    }

    target.extend_from_slice(lines);
    *last_segment_kind = Some(kind);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_closes_thought_block_when_text_arrives() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_thought_chunk("thinking");
        buffer.append_text_chunk("answer");
        buffer.append_thought_chunk("new thought");

        let segments: Vec<_> = buffer.segments().collect();
        assert_eq!(segments.len(), 3);
        assert!(matches!(segments[0], SegmentContent::Thought(_)));
        assert!(matches!(segments[1], SegmentContent::Text(_)));
        assert!(matches!(segments[2], SegmentContent::Thought(_)));
    }

    #[test]
    fn buffer_coalesces_contiguous_thought_chunks() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_thought_chunk("a");
        buffer.append_thought_chunk("b");

        let segments: Vec<_> = buffer.segments().collect();
        assert_eq!(segments.len(), 1);
        match segments[0] {
            SegmentContent::Thought(text) => assert_eq!(text, "ab"),
            _ => panic!("expected thought segment"),
        }
    }

    #[test]
    fn clear_removes_segments_and_resets_state() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_thought_chunk("thinking");
        buffer.append_text_chunk("answer");
        assert_eq!(buffer.segments().len(), 2);

        buffer.clear();

        assert_eq!(buffer.segments().len(), 0);
        buffer.append_thought_chunk("new");
        assert_eq!(buffer.segments().len(), 1);
    }

    #[test]
    fn drain_completed_returns_content_and_tool_ids() {
        use agent_client_protocol as acp;

        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        buffer.ensure_tool_segment("tool-1");

        let mut statuses = ToolCallStatuses::new();
        let tc = acp::ToolCall::new("tool-1", "Read file");
        statuses.on_tool_call(&tc);
        let update = acp::ToolCallUpdate::new(
            "tool-1",
            acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::Completed),
        );
        statuses.on_tool_call_update(&update);

        let (content, tool_ids) = buffer.drain_completed(&statuses);

        assert_eq!(content.len(), 2, "should have text and tool content");
        assert!(matches!(content[0], SegmentContent::Text(_)));
        assert!(matches!(content[1], SegmentContent::ToolCall(_)));
        assert_eq!(tool_ids, vec!["tool-1"]);
        assert_eq!(buffer.segments().len(), 0, "all segments should be drained");
    }

    #[test]
    fn drain_completed_keeps_running_tools() {
        use agent_client_protocol as acp;

        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        buffer.ensure_tool_segment("tool-1");

        let mut statuses = ToolCallStatuses::new();
        let tc = acp::ToolCall::new("tool-1", "Read file");
        statuses.on_tool_call(&tc);
        // tool-1 stays Running (no completion update)

        let (content, tool_ids) = buffer.drain_completed(&statuses);

        assert_eq!(content.len(), 1, "text segment should still be drained");
        assert!(matches!(content[0], SegmentContent::Text(_)));
        assert!(tool_ids.is_empty(), "running tool should not be drained");
        let segments: Vec<_> = buffer.segments().collect();
        assert_eq!(segments.len(), 1, "running tool should remain");
        assert!(matches!(
            segments[0],
            SegmentContent::ToolCall(id) if id == "tool-1"
        ));
    }
}
