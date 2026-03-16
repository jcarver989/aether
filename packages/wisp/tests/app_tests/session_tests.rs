use agent_client_protocol as acp;
use tui::testing::{TestTerminal, assert_buffer_eq};
use tui::{KeyCode, KeyEvent, KeyModifiers};

use super::common::*;

#[tokio::test]
async fn test_prompt_done_keeps_running_tool_segment() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    // Send a tool call that remains in-progress
    renderer
        .on_session_update(acp::SessionUpdate::ToolCall(acp::ToolCall::new(
            "tool-1",
            "Read file",
        )))
        .unwrap();

    renderer.on_prompt_done().unwrap();

    // The running tool should still be visible
    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Read file")),
        "Running tool should remain visible after prompt_done.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_prompt_done_flush_respects_rendering() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::AgentThoughtChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(
                "theme should be preserved",
            ))),
        ))
        .unwrap();

    renderer.on_prompt_done().unwrap();

    // Should render successfully
    let lines = renderer.writer().get_lines();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("theme should be preserved")),
        "Thought text should be visible after prompt_done.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_streaming_chunks_keep_waiting_for_response() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    // Submit prompt to enter waiting state
    type_string(&mut renderer, "Hello").await;
    press_enter(&mut renderer).await;

    // Send a streaming chunk (should not clear waiting state)
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("hello"))),
        ))
        .unwrap();

    // Escape should still trigger cancel (proving we're still waiting)
    let action = renderer
        .on_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await
        .unwrap();

    // If we're still waiting, escape triggers cancel effect which is handled
    assert!(matches!(action, LoopAction::Continue));
}

#[tokio::test]
async fn test_on_tick_without_active_state_is_noop() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    let lines_before = renderer.writer().get_lines();

    renderer.on_tick().await.unwrap();

    let lines_after = renderer.writer().get_lines();
    assert_eq!(
        lines_before, lines_after,
        "Tick should be a no-op when nothing active"
    );
}

#[tokio::test]
async fn test_in_progress_tool_call_visible_after_initial_render() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));

    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::ToolCall(
            acp::ToolCall::new("call_1".to_string(), "Read")
                .raw_input(serde_json::json!({"file": "test.rs"})),
        ))
        .unwrap();

    let expected = expected_with_prompt(
        &["⠒ Read", "⠒ (esc to interrupt)"],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_in_progress_tool_call_renders_correctly_after_resize() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], (TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::ToolCall(
            acp::ToolCall::new("call_1".to_string(), "Read")
                .raw_input(serde_json::json!({"file": "test.rs"})),
        ))
        .unwrap();

    // Terminal resize triggers full re-render at new width
    renderer.on_resize_event(100, 30).await.unwrap();

    let expected = expected_with_prompt(&["⠒ Read", "⠒ (esc to interrupt)"], 100, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}
