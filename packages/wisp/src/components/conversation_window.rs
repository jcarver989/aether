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
    /// Per-segment rendered-line cache for Text segments. Invalidated on mutation.
    cache: Vec<Option<Vec<Line>>>,
}

impl ConversationBuffer {
    pub(crate) fn new() -> Self {
        Self {
            segments: Vec::new(),
            thought_block_open: false,
            cache: Vec::new(),
        }
    }

    pub(crate) fn segments(&self) -> &[StreamSegment] {
        &self.segments
    }

    pub(crate) fn take_segments(&mut self) -> Vec<StreamSegment> {
        self.cache.clear();
        take(&mut self.segments)
    }

    pub(crate) fn set_segments(&mut self, segments: Vec<StreamSegment>) {
        self.cache = vec![None; segments.len()];
        self.segments = segments;
    }

    pub(crate) fn append_text_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        self.close_thought_block();

        if let Some(StreamSegment::Text(existing)) = self.segments.last_mut() {
            existing.push_str(chunk);
            if let Some(entry) = self.cache.last_mut() {
                *entry = None;
            }
        } else {
            self.segments.push(StreamSegment::Text(chunk.to_string()));
            self.cache.push(None);
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
        self.cache.push(None);
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
            self.cache.push(None);
        }
    }

    /// Re-render only the dirty text segments, caching the result.
    pub(crate) fn update_text_cache(&mut self, theme: &Theme) {
        for (i, segment) in self.segments.iter().enumerate() {
            if let StreamSegment::Text(text) = segment
                && self.cache[i].is_none()
            {
                self.cache[i] = Some(markdown::render_markdown(text, theme));
            }
        }
    }

    fn cached_lines(&self, index: usize) -> Option<&[Line]> {
        self.cache.get(index)?.as_deref()
    }
}

pub(crate) struct ConversationWindow<'a> {
    pub loader: &'a mut Spinner,
    pub conversation: &'a mut ConversationBuffer,
    pub tool_call_statuses: &'a ToolCallStatuses,
}

impl Component for ConversationWindow<'_> {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        self.conversation.update_text_cache(&context.theme);

        let mut lines = self.loader.render(context);
        let mut last_segment_kind: Option<StreamSegmentKind> = None;

        for (i, segment) in self.conversation.segments().iter().enumerate() {
            let kind = segment.kind();
            let segment_lines = if let Some(cached) = self.conversation.cached_lines(i) {
                cached.to_vec()
            } else {
                render_stream_segment(segment, self.tool_call_statuses, context)
            };
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
        let mut loader = Spinner::default();
        let mut conversation = ConversationBuffer::new();
        let statuses = ToolCallStatuses::new();
        let mut view = ConversationWindow {
            loader: &mut loader,
            conversation: &mut conversation,
            tool_call_statuses: &statuses,
        };
        let context = RenderContext::new((80, 24));

        let lines = view.render(&context);
        assert!(lines.is_empty());
    }

    #[test]
    fn inserts_vertical_margin_between_different_segment_kinds() {
        let mut loader = Spinner::default();
        let mut conversation = ConversationBuffer::new();
        conversation.set_segments(vec![
            StreamSegment::Text("one".to_string()),
            StreamSegment::Thought("two".to_string()),
            StreamSegment::Text("three".to_string()),
        ]);
        let statuses = ToolCallStatuses::new();
        let mut view = ConversationWindow {
            loader: &mut loader,
            conversation: &mut conversation,
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
        let mut loader = Spinner::default();
        let mut conversation = ConversationBuffer::new();
        conversation.set_segments(vec![
            StreamSegment::Text("first".to_string()),
            StreamSegment::Text("second".to_string()),
        ]);
        let statuses = ToolCallStatuses::new();
        let mut view = ConversationWindow {
            loader: &mut loader,
            conversation: &mut conversation,
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
        let mut conversation = ConversationBuffer::new();
        conversation.append_text_chunk("hello");
        let statuses = ToolCallStatuses::new();
        let mut view = ConversationWindow {
            loader: &mut loader,
            conversation: &mut conversation,
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

    #[test]
    fn update_text_cache_populates_cache_for_text_segments() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        buffer.append_thought_chunk("thinking");
        buffer.append_text_chunk("world");

        buffer.update_text_cache(&Theme::default());

        assert!(buffer.cached_lines(0).is_some(), "text segment should be cached");
        assert!(buffer.cached_lines(1).is_none(), "thought segment should not be cached");
        assert!(buffer.cached_lines(2).is_some(), "text segment should be cached");
    }

    #[test]
    fn append_text_chunk_invalidates_cache() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        buffer.update_text_cache(&Theme::default());
        assert!(buffer.cached_lines(0).is_some());

        // Append more text to the same segment — cache should be cleared.
        buffer.append_text_chunk(" world");
        assert!(buffer.cached_lines(0).is_none());
    }

    #[test]
    fn take_segments_clears_cache() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        buffer.update_text_cache(&Theme::default());

        let _ = buffer.take_segments();
        assert!(buffer.cached_lines(0).is_none());
    }

    #[test]
    fn set_segments_resets_cache() {
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("hello");
        buffer.update_text_cache(&Theme::default());

        buffer.set_segments(vec![
            StreamSegment::Text("a".to_string()),
            StreamSegment::Text("b".to_string()),
        ]);

        assert!(buffer.cached_lines(0).is_none());
        assert!(buffer.cached_lines(1).is_none());
    }

    #[test]
    fn render_uses_cached_lines() {
        let mut loader = Spinner::default();
        let mut buffer = ConversationBuffer::new();
        buffer.append_text_chunk("cached text");
        buffer.update_text_cache(&Theme::default());
        let statuses = ToolCallStatuses::new();
        let mut view = ConversationWindow {
            loader: &mut loader,
            conversation: &mut buffer,
            tool_call_statuses: &statuses,
        };
        let context = RenderContext::new((80, 24));

        let lines = view.render(&context);
        assert!(!lines.is_empty());
        assert!(lines[0].plain_text().contains("cached text"));
    }
}
