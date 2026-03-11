use std::mem::{Discriminant, discriminant, take};

use crate::components::thought_message::ThoughtMessage;
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::tui::{Line, Spinner, ViewContext, Widget, render_markdown};

#[derive(Debug, Clone)]
pub(crate) enum SegmentContent {
    Text(String),
    Thought(String),
    ToolCall(String),
}

#[derive(Debug)]
struct Segment {
    content: SegmentContent,
    lines: Option<Vec<Line>>,
}

pub(crate) struct ConversationBuffer {
    segments: Vec<Segment>,
    thought_block_open: bool,
}

impl ConversationBuffer {
    pub(crate) fn new() -> Self {
        Self {
            segments: Vec::new(),
            thought_block_open: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn segments(&self) -> impl ExactSizeIterator<Item = &SegmentContent> {
        self.segments.iter().map(|s| &s.content)
    }

    #[cfg(test)]
    pub(crate) fn set_segments(&mut self, segments: Vec<SegmentContent>) {
        self.segments = segments
            .into_iter()
            .map(|content| Segment {
                content,
                lines: None,
            })
            .collect();
    }

    pub(crate) fn append_text_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        self.close_thought_block();

        if let Some(segment) = self.segments.last_mut()
            && let SegmentContent::Text(existing) = &mut segment.content
        {
            existing.push_str(chunk);
            segment.lines = None;
        } else {
            self.segments.push(Segment {
                content: SegmentContent::Text(chunk.to_string()),
                lines: None,
            });
        }
    }

    pub(crate) fn append_thought_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        if self.thought_block_open
            && let Some(segment) = self.segments.last_mut()
            && let SegmentContent::Thought(existing) = &mut segment.content
        {
            existing.push_str(chunk);
            segment.lines = None;

            return;
        }

        self.segments.push(Segment {
            content: SegmentContent::Thought(chunk.to_string()),
            lines: None,
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
                lines: None,
            });
        }
    }

    pub(crate) fn invalidate_tool_segment(&mut self, tool_id: &str) {
        for segment in &mut self.segments {
            if matches!(&segment.content, SegmentContent::ToolCall(id) if id == tool_id) {
                segment.lines = None;
            }
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

    pub(crate) fn flush_completed(
        &mut self,
        tool_call_statuses: &ToolCallStatuses,
        context: &ViewContext,
    ) -> (Vec<Line>, Vec<String>) {
        let drained = self.drain_segments_except(|seg| {
            matches!(seg, SegmentContent::ToolCall(id) if tool_call_statuses.is_tool_running(id))
        });

        let mut scrollback_lines: Vec<Line> = Vec::new();
        let mut last_segment_kind = None;
        let mut completed_tool_ids = Vec::new();

        for segment in drained {
            let kind = discriminant(&segment.content);
            let lines = segment.lines.unwrap_or_else(|| {
                render_stream_segment(&segment.content, tool_call_statuses, context)
            });
            extend_with_vertical_margin(
                &mut scrollback_lines,
                &mut last_segment_kind,
                kind,
                &lines,
            );
            if let SegmentContent::ToolCall(id) = segment.content {
                completed_tool_ids.push(id);
            }
        }

        (scrollback_lines, completed_tool_ids)
    }

    fn segments_len(&self) -> usize {
        self.segments.len()
    }

    /// Pre-renders all segments so that `get_cached` can serve them
    /// from an immutable reference during `render()`.
    pub(crate) fn ensure_all_rendered(
        &mut self,
        tool_call_statuses: &ToolCallStatuses,
        context: &ViewContext,
    ) {
        self.invalidate_active_tool_lines(tool_call_statuses);
        for i in 0..self.segments.len() {
            self.get_or_render(i, tool_call_statuses, context);
        }
    }

    /// Returns cached rendered lines for segment `i`.
    /// Panics if `ensure_all_rendered` was not called first.
    fn get_cached(&self, i: usize) -> (Discriminant<SegmentContent>, &[Line]) {
        let segment = &self.segments[i];
        (
            discriminant(&segment.content),
            segment
                .lines
                .as_deref()
                .expect("ensure_all_rendered must be called before render"),
        )
    }

    /// Clears cached lines for active `ToolCall` segments so spinners keep
    /// animating while completed tool output stays cached.
    fn invalidate_active_tool_lines(&mut self, tool_call_statuses: &ToolCallStatuses) {
        for segment in &mut self.segments {
            if let SegmentContent::ToolCall(ref id) = segment.content
                && tool_call_statuses.is_tool_active_for_render(id)
            {
                segment.lines = None;
            }
        }
    }

    /// Populates and returns the cached rendered lines for segment `i`,
    /// along with its discriminant for vertical-margin logic.
    fn get_or_render(
        &mut self,
        i: usize,
        tool_call_statuses: &ToolCallStatuses,
        context: &ViewContext,
    ) -> (Discriminant<SegmentContent>, &[Line]) {
        if self.segments[i].lines.is_none() {
            let rendered =
                render_stream_segment(&self.segments[i].content, tool_call_statuses, context);
            self.segments[i].lines = Some(rendered);
        }
        (
            discriminant(&self.segments[i].content),
            self.segments[i].lines.as_deref().unwrap(),
        )
    }

    #[cfg(test)]
    fn cached_lines(&self, index: usize) -> Option<&[Line]> {
        self.segments.get(index)?.lines.as_deref()
    }
}

pub(crate) struct ConversationWindow<'a> {
    pub loader: &'a Spinner,
    pub conversation: &'a ConversationBuffer,
}

impl ConversationWindow<'_> {
    pub fn render(&self, context: &ViewContext) -> Vec<Line> {
        let mut lines = self.loader.render(context);
        let mut last_segment_kind = None;

        for i in 0..self.conversation.segments_len() {
            let (kind, cached) = self.conversation.get_cached(i);
            extend_with_vertical_margin(&mut lines, &mut last_segment_kind, kind, cached);
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
    use crate::tui::BRAILLE_FRAMES;

    #[test]
    fn renders_empty_when_loader_and_segments_are_empty() {
        let loader = Spinner::default();
        let mut conversation = ConversationBuffer::new();
        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));
        conversation.ensure_all_rendered(&statuses, &context);
        let view = ConversationWindow {
            loader: &loader,
            conversation: &conversation,
        };

        let lines = view.render(&context);
        assert!(lines.is_empty());
    }

    #[test]
    fn inserts_vertical_margin_between_different_segment_kinds() {
        let loader = Spinner::default();
        let mut conversation = ConversationBuffer::new();
        conversation.set_segments(vec![
            SegmentContent::Text("one".to_string()),
            SegmentContent::Thought("two".to_string()),
            SegmentContent::Text("three".to_string()),
        ]);
        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));
        conversation.ensure_all_rendered(&statuses, &context);
        let view = ConversationWindow {
            loader: &loader,
            conversation: &conversation,
        };

        let lines = view.render(&context);
        assert_eq!(lines.len(), 5);
        assert!(lines[0].plain_text().contains("one"));
        assert_eq!(lines[1].plain_text(), "");
        assert!(lines[2].plain_text().starts_with("│ "));
        assert!(lines[2].plain_text().contains("two"));
        assert_eq!(lines[3].plain_text(), "");
        assert!(lines[4].plain_text().contains("three"));
    }

    #[test]
    fn does_not_insert_vertical_margin_for_same_kind_segments() {
        let loader = Spinner::default();
        let mut conversation = ConversationBuffer::new();
        conversation.set_segments(vec![
            SegmentContent::Text("first".to_string()),
            SegmentContent::Text("second".to_string()),
        ]);
        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));
        conversation.ensure_all_rendered(&statuses, &context);
        let view = ConversationWindow {
            loader: &loader,
            conversation: &conversation,
        };

        let lines = view.render(&context);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("first"));
        assert!(lines[1].plain_text().contains("second"));
    }

    #[test]
    fn renders_loader_before_segments() {
        let mut loader = Spinner::default();
        loader.visible = true;
        let mut conversation = ConversationBuffer::new();
        conversation.append_text_chunk("hello");
        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));
        conversation.ensure_all_rendered(&statuses, &context);
        let view = ConversationWindow {
            loader: &loader,
            conversation: &conversation,
        };

        let lines = view.render(&context);
        assert_eq!(lines.len(), 2);
        let loader_line = lines[0].plain_text();
        assert!(
            BRAILLE_FRAMES
                .iter()
                .any(|frame| loader_line.contains(frame.to_string().as_str()))
        );
        assert!(lines[1].plain_text().contains("hello"));
    }

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
    fn get_or_render_populates_cache_for_all_segments() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        buffer.append_thought_chunk("thinking");
        buffer.append_text_chunk("world");

        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));

        assert!(
            !buffer.get_or_render(0, &statuses, &context).1.is_empty(),
            "text segment should return lines"
        );
        assert!(
            !buffer.get_or_render(1, &statuses, &context).1.is_empty(),
            "thought segment should return lines"
        );
        assert!(
            !buffer.get_or_render(2, &statuses, &context).1.is_empty(),
            "text segment should return lines"
        );

        // Verify lines are cached for subsequent reads.
        assert!(buffer.cached_lines(0).is_some());
        assert!(buffer.cached_lines(1).is_some());
        assert!(buffer.cached_lines(2).is_some());
    }

    #[test]
    fn append_text_chunk_invalidates_cache() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));
        buffer.get_or_render(0, &statuses, &context);
        assert!(buffer.cached_lines(0).is_some());

        // Append more text to the same segment — cache should be cleared.
        buffer.append_text_chunk(" world");
        assert!(buffer.cached_lines(0).is_none());
    }

    #[test]
    fn flush_completed_clears_cache() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));
        buffer.get_or_render(0, &statuses, &context);

        let _ = buffer.flush_completed(&statuses, &context);
        assert!(buffer.cached_lines(0).is_none());
    }

    #[test]
    fn set_segments_resets_cache() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));
        buffer.get_or_render(0, &statuses, &context);

        buffer.set_segments(vec![
            SegmentContent::Text("a".to_string()),
            SegmentContent::Text("b".to_string()),
        ]);

        assert!(buffer.cached_lines(0).is_none());
        assert!(buffer.cached_lines(1).is_none());
    }

    #[test]
    fn render_uses_cached_lines() {
        let loader = Spinner::default();
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("cached text");
        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));
        buffer.get_or_render(0, &statuses, &context);
        buffer.ensure_all_rendered(&statuses, &context);
        let view = ConversationWindow {
            loader: &loader,
            conversation: &buffer,
        };

        let lines = view.render(&context);
        assert!(!lines.is_empty());
        assert!(lines[0].plain_text().contains("cached text"));
    }

    #[test]
    fn invalidate_active_tool_lines_clears_only_running_tool_segments() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("text");
        buffer.append_thought_chunk("thought");
        buffer.ensure_tool_segment("tool-1");
        buffer.ensure_tool_segment("tool-2");

        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&agent_client_protocol::ToolCall::new("tool-1", "Read file"));
        statuses.on_tool_call(&agent_client_protocol::ToolCall::new("tool-2", "Read file"));
        statuses.on_tool_call_update(&agent_client_protocol::ToolCallUpdate::new(
            "tool-2",
            agent_client_protocol::ToolCallUpdateFields::new()
                .status(agent_client_protocol::ToolCallStatus::Completed),
        ));
        let context = ViewContext::new((80, 24));
        buffer.get_or_render(0, &statuses, &context);
        buffer.get_or_render(1, &statuses, &context);
        buffer.get_or_render(2, &statuses, &context);
        buffer.get_or_render(3, &statuses, &context);
        assert!(buffer.cached_lines(0).is_some());
        assert!(buffer.cached_lines(1).is_some());
        assert!(buffer.cached_lines(2).is_some());
        assert!(buffer.cached_lines(3).is_some());

        buffer.invalidate_active_tool_lines(&statuses);

        assert!(
            buffer.cached_lines(0).is_some(),
            "text cache should survive"
        );
        assert!(
            buffer.cached_lines(1).is_some(),
            "thought cache should survive"
        );
        assert!(
            buffer.cached_lines(2).is_none(),
            "running tool cache should be cleared"
        );
        assert!(
            buffer.cached_lines(3).is_some(),
            "completed tool cache should survive"
        );
    }

    #[test]
    fn invalidate_active_tool_lines_keeps_completed_sub_agent_segments_cached() {
        let mut buffer = ConversationBuffer::new();
        buffer.ensure_tool_segment("parent-1");

        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&agent_client_protocol::ToolCall::new(
            "parent-1",
            "spawn_subagent",
        ));
        statuses.on_tool_call_update(&agent_client_protocol::ToolCallUpdate::new(
            "parent-1",
            agent_client_protocol::ToolCallUpdateFields::new()
                .status(agent_client_protocol::ToolCallStatus::Completed),
        ));
        statuses.on_sub_agent_progress(
            &serde_json::from_str(
                r#"{"parent_tool_id":"parent-1","task_id":"task-1","agent_name":"explorer","event":{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}}"#,
            )
            .unwrap(),
        );
        statuses.on_sub_agent_progress(
            &serde_json::from_str(
                r#"{"parent_tool_id":"parent-1","task_id":"task-1","agent_name":"explorer","event":{"ToolResult":{"result":{"id":"c1","name":"grep","arguments":"{}","result":"ok"},"model_name":"m"}}}"#,
            )
            .unwrap(),
        );
        statuses.on_sub_agent_progress(
            &serde_json::from_str(
                r#"{"parent_tool_id":"parent-1","task_id":"task-1","agent_name":"explorer","event":"Done"}"#,
            )
            .unwrap(),
        );

        let context = ViewContext::new((80, 24));
        buffer.get_or_render(0, &statuses, &context);
        assert!(buffer.cached_lines(0).is_some());

        buffer.invalidate_active_tool_lines(&statuses);

        assert!(
            buffer.cached_lines(0).is_some(),
            "completed sub-agent segment should stay cached"
        );
    }

    #[test]
    fn append_thought_chunk_invalidates_cache() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_thought_chunk("first");

        let statuses = ToolCallStatuses::new();
        let context = ViewContext::new((80, 24));
        buffer.get_or_render(0, &statuses, &context);
        assert!(buffer.cached_lines(0).is_some());

        buffer.append_thought_chunk(" more");
        assert!(
            buffer.cached_lines(0).is_none(),
            "cache should be cleared on append"
        );
    }

    #[test]
    fn invalidate_tool_segment_clears_only_matching_tool_cache() {
        let mut buffer = ConversationBuffer::new();
        buffer.ensure_tool_segment("tool-1");
        buffer.ensure_tool_segment("tool-2");

        let mut statuses = ToolCallStatuses::new();
        statuses.on_tool_call(&agent_client_protocol::ToolCall::new("tool-1", "Read file"));
        statuses.on_tool_call(&agent_client_protocol::ToolCall::new("tool-2", "Read file"));
        let context = ViewContext::new((80, 24));

        buffer.get_or_render(0, &statuses, &context);
        buffer.get_or_render(1, &statuses, &context);
        assert!(buffer.cached_lines(0).is_some());
        assert!(buffer.cached_lines(1).is_some());

        buffer.invalidate_tool_segment("tool-2");

        assert!(buffer.cached_lines(0).is_some());
        assert!(buffer.cached_lines(1).is_none());
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
    fn flush_completed_returns_lines_and_tool_ids() {
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

        let context = ViewContext::new((80, 24));
        let (lines, tool_ids) = buffer.flush_completed(&statuses, &context);

        assert!(!lines.is_empty(), "should produce scrollback lines");
        assert_eq!(tool_ids, vec!["tool-1"]);
        assert_eq!(buffer.segments().len(), 0, "all segments should be drained");
    }

    #[test]
    fn flush_completed_keeps_running_tools() {
        use agent_client_protocol as acp;

        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        buffer.ensure_tool_segment("tool-1");

        let mut statuses = ToolCallStatuses::new();
        let tc = acp::ToolCall::new("tool-1", "Read file");
        statuses.on_tool_call(&tc);
        // tool-1 stays Running (no completion update)

        let context = ViewContext::new((80, 24));
        let (lines, tool_ids) = buffer.flush_completed(&statuses, &context);

        assert!(!lines.is_empty(), "text segment should still produce lines");
        assert!(tool_ids.is_empty(), "running tool should not be drained");
        let segments: Vec<_> = buffer.segments().collect();
        assert_eq!(segments.len(), 1, "running tool should remain");
        assert!(matches!(
            segments[0],
            SegmentContent::ToolCall(id) if id == "tool-1"
        ));
    }

    #[test]
    fn flush_completed_reuses_cached_lines() {
        use agent_client_protocol as acp;

        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("cached");

        let mut statuses = ToolCallStatuses::new();
        let tc = acp::ToolCall::new("tool-1", "Read file");
        statuses.on_tool_call(&tc);
        buffer.ensure_tool_segment("tool-1");
        let update = acp::ToolCallUpdate::new(
            "tool-1",
            acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::Completed),
        );
        statuses.on_tool_call_update(&update);

        let context = ViewContext::new((80, 24));

        // Pre-render to populate cache for the text segment.
        buffer.get_or_render(0, &statuses, &context);
        assert!(buffer.cached_lines(0).is_some());

        let (lines, tool_ids) = buffer.flush_completed(&statuses, &context);

        assert!(!lines.is_empty());
        assert!(lines.iter().any(|l| l.plain_text().contains("cached")));
        assert_eq!(tool_ids, vec!["tool-1"]);
    }
}
