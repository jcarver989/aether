mod test_terminal;

use aether::agent::AgentMessage;
use aether::llm::{ToolCallRequest, ToolCallResult};
use test_terminal::{TestTerminal, assert_buffer_eq};
use wisp::colors::Theme;
use wisp::renderer::Renderer;

#[tokio::test]
async fn test_agent_message_text_chunks() {
    let (renderer, _theme) = render(vec![
        text_chunk("Hello"),
        text_chunk(" World"),
        text_complete(""), // Signal completion
    ])
    .await;

    assert_buffer_eq(renderer.writer(), &["Hello World", ">"]);
}

#[tokio::test]
async fn test_agent_message_tool_call() {
    let (renderer, _theme) = render(vec![tool_call("test_tool", r#"{"arg1": "value1"}"#)]).await;

    assert_buffer_eq(
        renderer.writer(),
        &["● test_tool {\"arg1\": \"value1\"}", ">"],
    );
}

#[tokio::test]
async fn test_agent_message_tool_result() {
    let args = r#"{"arg1": "value1"}"#;
    let (renderer, _theme) = render(vec![
        tool_call("test_tool", args),
        tool_result("test_tool", args, "success"),
    ])
    .await;

    // The tool result should have overwritten the tool call on the same line
    // So we should only see the success message with a single prompt
    assert_buffer_eq(
        renderer.writer(),
        &["● test_tool ✓ {\"arg1\": \"value1\"}", ">"],
    );
}

#[tokio::test]
async fn test_multiple_messages_sequence() {
    let args = r#"{"query": "test"}"#;
    let (renderer, _theme) = render(vec![
        text_complete("Processing your request"),
        tool_call("search", args),
        tool_result("search", args, "found items"),
        text_complete("Found results"),
    ])
    .await;

    // The tool call clears the previous prompt line, and the tool result
    // updates the tool call line in place without adding a new prompt
    assert_buffer_eq(
        renderer.writer(),
        &[
            "Processing your request",
            "● search ✓ {\"query\": \"test\"}",
            "  Found results", // Has leading spaces from clear line position
            ">",
        ],
    );
}

#[tokio::test]
async fn test_streaming_tool_call_arguments() {
    let args1 = r#"{"file":"#;
    let args2 = r#"{"file": "test.rs"#;
    let args_complete = r#"{"file": "test.rs"}"#;

    let (renderer, _theme) = render(vec![
        // Simulate streaming arguments - same tool ID, arguments build up
        tool_call_with_id("Read", "call_1", args1),
        tool_call_with_id("Read", "call_1", args2),
        tool_call_with_id("Read", "call_1", args_complete),
        tool_result_with_id("Read", "call_1", args_complete, "file contents"),
    ])
    .await;

    // Should only render one line for the tool call, updated as arguments stream in
    // The duplicate tool call detection should prevent multiple lines
    assert_buffer_eq(
        renderer.writer(),
        &["● Read ✓ {\"file\": \"test.rs\"}", ">"],
    );
}

#[tokio::test]
async fn test_multiple_parallel_tool_calls() {
    let args1 = r#"{"file": "test.rs"}"#;
    let args2 = r#"{"pattern": "foo"}"#;
    let args3 = r#"{"path": "src/"}"#;

    let (renderer, _theme) = render(vec![
        tool_call("Read", args1),
        tool_call("Grep", args2),
        tool_call("Glob", args3),
        tool_result("Read", args1, "file contents"),
        tool_result("Grep", args2, "matches"),
        tool_result("Glob", args3, "files"),
    ])
    .await;

    // Each tool call should render on its own line with the last one adding a prompt
    // Tool results should update their respective lines in-place without adding new prompts
    assert_buffer_eq(
        renderer.writer(),
        &[
            "● Read ✓ {\"file\": \"test.rs\"}",
            "● Grep ✓ {\"pattern\": \"foo\"}",
            "● Glob ✓ {\"path\": \"src/\"}",
            ">",
        ],
    );
}

async fn render(messages: Vec<AgentMessage>) -> (Renderer<TestTerminal>, Theme) {
    let terminal = TestTerminal::new(200, 40);
    let mut renderer = Renderer::new(terminal);

    for msg in messages {
        // Update context with current terminal state
        let position = renderer.writer().cursor_position();
        let size = renderer.writer().size();
        renderer.update_render_context_with(position, size);
        renderer.on_agent_message(msg).await.unwrap();
    }

    (renderer, Theme::default())
}

#[tokio::test]
async fn test_user_message_submission() {
    use tokio::sync::mpsc;

    let terminal = TestTerminal::new(200, 40);
    let mut renderer = Renderer::new(terminal);
    let (tx, _rx) = mpsc::channel(10);

    // Update context with current terminal state
    let position = renderer.writer().cursor_position();
    let size = renderer.writer().size();
    renderer.update_render_context_with(position, size);

    // Simulate typing "Hello world" and pressing Enter
    type_string(&mut renderer, "Hello world", &tx).await;
    press_enter(&mut renderer, &tx).await;

    // Verify the terminal output shows the user's message followed by a new prompt
    // Note: InputPrompt adds a blank line (\r\n) before the prompt
    assert_buffer_eq(
        renderer.writer(),
        &[
            "Hello world",
            "", // Blank line from InputPrompt's \r\n
            ">",
        ],
    );
}

fn text_chunk(chunk: &str) -> AgentMessage {
    AgentMessage::Text {
        message_id: "test_msg".to_string(),
        chunk: chunk.to_string(),
        is_complete: false,
        model_name: "test".to_string(),
    }
}

fn text_complete(text: &str) -> AgentMessage {
    AgentMessage::Text {
        message_id: "test_msg".to_string(),
        chunk: text.to_string(),
        is_complete: true,
        model_name: "test".to_string(),
    }
}

fn tool_call(name: &str, args: &str) -> AgentMessage {
    tool_call_with_id(name, &format!("call_{}", name), args)
}

fn tool_call_with_id(name: &str, id: &str, args: &str) -> AgentMessage {
    AgentMessage::ToolCall {
        request: ToolCallRequest {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args.to_string(),
        },
        model_name: "test".to_string(),
    }
}

fn tool_result(name: &str, args: &str, result: &str) -> AgentMessage {
    tool_result_with_id(name, &format!("call_{}", name), args, result)
}

fn tool_result_with_id(name: &str, id: &str, args: &str, result: &str) -> AgentMessage {
    AgentMessage::ToolResult {
        result: ToolCallResult {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args.to_string(),
            result: result.to_string(),
        },
        model_name: "test".to_string(),
    }
}

async fn type_string<W: std::io::Write>(
    renderer: &mut Renderer<W>,
    text: &str,
    tx: &tokio::sync::mpsc::Sender<aether::agent::UserMessage>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    for ch in text.chars() {
        let key_event = KeyEvent {
            code: KeyCode::Char(ch),
            modifiers: KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        };
        renderer.on_key_event(key_event, tx).await.unwrap();
    }
}

async fn press_enter<W: std::io::Write>(
    renderer: &mut Renderer<W>,
    tx: &tokio::sync::mpsc::Sender<aether::agent::UserMessage>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let enter_event = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::empty(),
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    };
    renderer.on_key_event(enter_event, tx).await.unwrap();
}
