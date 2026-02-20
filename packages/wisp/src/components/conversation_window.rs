use std::mem::take;

use crate::components::thought_message::ThoughtMessage;
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::tui::markdown;
use crate::tui::spinner::Spinner;
use crate::tui::theme::Theme;
use crate::tui::{Component, Line, RenderContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamSegmentKind {
    Text,
    Thought,
    ToolCall,
}

#[derive(Debug, Clone)]
pub(crate) enum StreamSegment {
    Text(String),
    Thought(String),
    ToolCall(String),
}

impl StreamSegment {
    pub(crate) fn kind(&self) -> StreamSegmentKind {
        match self {
            Self::Text(_) => StreamSegmentKind::Text,
            Self::Thought(_) => StreamSegmentKind::Thought,
            Self::ToolCall(_) => StreamSegmentKind::ToolCall,
        }
    }
}

pub(crate) struct ConversationBuffer {
    segments: Vec<StreamSegment>,
    thought_block_open: bool,
}

impl ConversationBuffer {
    pub(crate) fn new() -> Self {
        Self {
            segments: Vec::new(),
            thought_block_open: false,
        }
    }

    pub(crate) fn segments(&self) -> &[StreamSegment] {
        &self.segments
    }

    pub(crate) fn take_segments(&mut self) -> Vec<StreamSegment> {
        take(&mut self.segments)
    }

    pub(crate) fn set_segments(&mut self, segments: Vec<StreamSegment>) {
        self.segments = segments;
    }

    pub(crate) fn append_text_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        self.close_thought_block();

        match self.segments.last_mut() {
            Some(StreamSegment::Text(existing)) => existing.push_str(chunk),
            _ => self.segments.push(StreamSegment::Text(chunk.to_string())),
        }
    }

    pub(crate) fn append_thought_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        if self.thought_block_open
            && let Some(StreamSegment::Thought(existing)) = self.segments.last_mut()
        {
            existing.push_str(chunk);
            return;
        }

        self.segments
            .push(StreamSegment::Thought(chunk.to_string()));
        self.thought_block_open = true;
    }

    pub(crate) fn close_thought_block(&mut self) {
        self.thought_block_open = false;
    }

    pub(crate) fn ensure_tool_segment(&mut self, tool_id: &str) {
        let has_segment = self
            .segments
            .iter()
            .any(|segment| matches!(segment, StreamSegment::ToolCall(id) if id == tool_id));

        if !has_segment {
            self.segments
                .push(StreamSegment::ToolCall(tool_id.to_string()));
        }
    }
}

pub(crate) struct ConversationWindow<'a> {
    pub loader: &'a Spinner,
    pub segments: &'a [StreamSegment],
    pub tool_call_statuses: &'a ToolCallStatuses,
}

impl Component for ConversationWindow<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = self.loader.render(context);
        let mut last_segment_kind: Option<StreamSegmentKind> = None;

        for segment in self.segments {
            let kind = segment.kind();
            let segment_lines = render_stream_segment(segment, self.tool_call_statuses, context);
            extend_with_vertical_margin(&mut lines, &mut last_segment_kind, kind, segment_lines);
        }

        lines
    }
}

pub(crate) fn render_stream_segment(
    segment: &StreamSegment,
    tool_call_statuses: &ToolCallStatuses,
    context: &RenderContext,
) -> Vec<Line> {
    match segment {
        StreamSegment::Thought(text) => ThoughtMessage { text }.render(context),
        StreamSegment::Text(text) => render_text_segment(text, &context.theme),
        StreamSegment::ToolCall(id) => tool_call_statuses.render_tool(id, context),
    }
}

pub(crate) fn extend_with_vertical_margin(
    target: &mut Vec<Line>,
    last_segment_kind: &mut Option<StreamSegmentKind>,
    kind: StreamSegmentKind,
    lines: Vec<Line>,
) {
    if lines.is_empty() {
        return;
    }

    if let Some(prev_kind) = *last_segment_kind
        && prev_kind != kind
    {
        target.push(Line::new(String::new()));
    }

    target.extend(lines);
    *last_segment_kind = Some(kind);
}

fn render_text_segment(text: &str, theme: &Theme) -> Vec<Line> {
    if text.is_empty() {
        return vec![];
    }

    markdown::render_markdown(text, theme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::spinner::BRAILLE_FRAMES;

    #[test]
    fn renders_empty_when_loader_and_segments_are_empty() {
        let loader = Spinner::default();
        let statuses = ToolCallStatuses::new();
        let view = ConversationWindow {
            loader: &loader,
            segments: &[],
            tool_call_statuses: &statuses,
        };
        let context = RenderContext::new((80, 24));

        let lines = view.render(&context);
        assert!(lines.is_empty());
    }

    #[test]
    fn inserts_vertical_margin_between_different_segment_kinds() {
        let loader = Spinner::default();
        let statuses = ToolCallStatuses::new();
        let segments = vec![
            StreamSegment::Text("one".to_string()),
            StreamSegment::Thought("two".to_string()),
            StreamSegment::Text("three".to_string()),
        ];
        let view = ConversationWindow {
            loader: &loader,
            segments: &segments,
            tool_call_statuses: &statuses,
        };
        let context = RenderContext::new((80, 24));

        let lines = view.render(&context);
        assert_eq!(lines.len(), 5);
        assert!(lines[0].plain_text().contains("one"));
        assert_eq!(lines[1].plain_text(), "");
        assert!(lines[2].plain_text().contains("Thought:"));
        assert!(lines[2].plain_text().contains("two"));
        assert_eq!(lines[3].plain_text(), "");
        assert!(lines[4].plain_text().contains("three"));
    }

    #[test]
    fn does_not_insert_vertical_margin_for_same_kind_segments() {
        let loader = Spinner::default();
        let statuses = ToolCallStatuses::new();
        let segments = vec![
            StreamSegment::Text("first".to_string()),
            StreamSegment::Text("second".to_string()),
        ];
        let view = ConversationWindow {
            loader: &loader,
            segments: &segments,
            tool_call_statuses: &statuses,
        };
        let context = RenderContext::new((80, 24));

        let lines = view.render(&context);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("first"));
        assert!(lines[1].plain_text().contains("second"));
    }

    #[test]
    fn renders_loader_before_segments() {
        let mut loader = Spinner::default();
        loader.visible = true;
        let statuses = ToolCallStatuses::new();
        let segments = vec![StreamSegment::Text("hello".to_string())];
        let view = ConversationWindow {
            loader: &loader,
            segments: &segments,
            tool_call_statuses: &statuses,
        };
        let context = RenderContext::new((80, 24));

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

        let segments = buffer.segments();
        assert_eq!(segments.len(), 3);
        assert!(matches!(segments[0], StreamSegment::Thought(_)));
        assert!(matches!(segments[1], StreamSegment::Text(_)));
        assert!(matches!(segments[2], StreamSegment::Thought(_)));
    }

    #[test]
    fn buffer_coalesces_contiguous_thought_chunks() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_thought_chunk("a");
        buffer.append_thought_chunk("b");

        let segments = buffer.segments();
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            StreamSegment::Thought(text) => assert_eq!(text, "ab"),
            _ => panic!("expected thought segment"),
        }
    }
}
