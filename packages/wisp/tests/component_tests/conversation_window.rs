use tui::testing::render_lines;
use tui::{Spinner, ViewContext, BRAILLE_FRAMES};
use wisp::components::conversation_window::{ConversationBuffer, ConversationWindow};
use wisp::components::tool_call_statuses::ToolCallStatuses;

#[test]
fn renders_empty_when_loader_and_segments_are_empty() {
    let mut loader = Spinner::default();
    let conversation = ConversationBuffer::new();
    let statuses = ToolCallStatuses::new();
    let context = ViewContext::new((80, 24));
    let mut view = ConversationWindow {
        loader: &mut loader,
        conversation: &conversation,
        tool_call_statuses: &statuses,
    };

    let lines = view.render(&context);
    assert!(lines.is_empty());
}

#[test]
fn inserts_vertical_margin_between_different_segment_kinds() {
    let mut loader = Spinner::default();
    let mut conversation = ConversationBuffer::new();
    conversation.append_text_chunk("one");
    conversation.append_thought_chunk("two");
    conversation.append_text_chunk("three");
    let statuses = ToolCallStatuses::new();
    let context = ViewContext::new((80, 24));
    let mut view = ConversationWindow {
        loader: &mut loader,
        conversation: &conversation,
        tool_call_statuses: &statuses,
    };

    let lines = view.render(&context);
    assert_eq!(lines.len(), 5);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("one"));
    assert_eq!(output[1], "");
    assert!(output[2].starts_with("\u{2502} "));
    assert!(output[2].contains("two"));
    assert_eq!(output[3], "");
    assert!(output[4].contains("three"));
}

#[test]
fn consecutive_text_chunks_render_without_margin() {
    let mut loader = Spinner::default();
    let mut conversation = ConversationBuffer::new();
    conversation.append_text_chunk("first ");
    conversation.append_text_chunk("second");
    let statuses = ToolCallStatuses::new();
    let context = ViewContext::new((80, 24));
    let mut view = ConversationWindow {
        loader: &mut loader,
        conversation: &conversation,
        tool_call_statuses: &statuses,
    };

    let lines = view.render(&context);
    // Consecutive text chunks are coalesced, so there should be one line with no margin
    assert_eq!(lines.len(), 1);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(output[0].contains("first"), "text: {}", output[0]);
    assert!(output[0].contains("second"), "text: {}", output[0]);
}

#[test]
fn renders_loader_before_segments() {
    let mut loader = Spinner::default();
    loader.visible = true;
    let mut conversation = ConversationBuffer::new();
    conversation.append_text_chunk("hello");
    let statuses = ToolCallStatuses::new();
    let context = ViewContext::new((80, 24));
    let mut view = ConversationWindow {
        loader: &mut loader,
        conversation: &conversation,
        tool_call_statuses: &statuses,
    };

    let lines = view.render(&context);
    assert_eq!(lines.len(), 2);
    let term = render_lines(&lines, 80, 24);
    let output = term.get_lines();
    assert!(
        BRAILLE_FRAMES
            .iter()
            .any(|frame| output[0].contains(frame.to_string().as_str()))
    );
    assert!(output[1].contains("hello"));
}
