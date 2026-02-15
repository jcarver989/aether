mod test_terminal;

use aether::llm::{ToolCallRequest, ToolCallResult};
use agent_events::{AgentMessage, UserMessage};
use test_terminal::{TestTerminal, assert_buffer_eq};
use wisp::renderer::Renderer;

#[tokio::test]
async fn test_agent_message_text_chunks() {
    let renderer = render(vec![
        text_chunk("Hello"),
        text_chunk(" World"),
        text_complete(""),
    ])
    .await;

    assert_buffer_eq(renderer.writer(), &["Hello World", ">"]);
}

#[tokio::test]
async fn test_agent_message_tool_call() {
    let renderer = render(vec![tool_call("test_tool", r#"{"arg1": "value1"}"#)]).await;

    assert_buffer_eq(
        renderer.writer(),
        &["● test_tool {\"arg1\": \"value1\"}", ">"],
    );
}

#[tokio::test]
async fn test_agent_message_tool_result() {
    let args = r#"{"arg1": "value1"}"#;
    let renderer = render(vec![
        tool_call("test_tool", args),
        tool_result("test_tool", args, "success"),
    ])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &["● test_tool ✓ {\"arg1\": \"value1\"}", ">"],
    );
}

#[tokio::test]
async fn test_multiple_messages_sequence() {
    let args = r#"{"query": "test"}"#;
    let renderer = render(vec![
        text_complete("Processing your request"),
        tool_call("search", args),
        tool_result("search", args, "found items"),
        text_complete("Found results"),
    ])
    .await;

    // After the first text_complete, tool calls + text are pushed to scrollback.
    // Then tool_call + tool_result happen, then text_complete pushes everything to scrollback again.
    assert_buffer_eq(
        renderer.writer(),
        &[
            "Processing your request",
            "● search ✓ {\"query\": \"test\"}",
            "Found results",
            ">",
        ],
    );
}

#[tokio::test]
async fn test_streaming_tool_call_arguments() {
    let args1 = r#"{"file":"#;
    let args2 = r#"{"file": "test.rs"#;
    let args_complete = r#"{"file": "test.rs"}"#;

    let renderer = render(vec![
        tool_call_with_id("Read", "call_1", args1),
        tool_call_with_id("Read", "call_1", args2),
        tool_call_with_id("Read", "call_1", args_complete),
        tool_result_with_id("Read", "call_1", args_complete, "file contents"),
    ])
    .await;

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

    let renderer = render(vec![
        tool_call("Read", args1),
        tool_call("Grep", args2),
        tool_call("Glob", args3),
        tool_result("Read", args1, "file contents"),
        tool_result("Grep", args2, "matches"),
        tool_result("Glob", args3, "files"),
    ])
    .await;

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

async fn render(messages: Vec<AgentMessage>) -> Renderer<TestTerminal> {
    let terminal = TestTerminal::new(200, 40);
    let mut renderer = Renderer::new(terminal, 0);
    renderer.update_render_context_with((200, 40));

    for msg in messages {
        renderer.on_agent_message(msg).await.unwrap();
    }

    renderer
}

#[tokio::test]
async fn test_user_message_submission() {
    use tokio::sync::mpsc;

    let terminal = TestTerminal::new(200, 40);
    let mut renderer = Renderer::new(terminal, 0);
    renderer.update_render_context_with((200, 40));

    let (tx, _rx) = mpsc::channel(10);

    // Render initial prompt
    renderer.initial_render().unwrap();

    // Simulate typing "Hello world" and pressing Enter
    type_string(&mut renderer, "Hello world", &tx).await;
    press_enter(&mut renderer, &tx).await;

    // push_to_scrollback clears the managed region (row 0 prompt), writes "Hello world"
    // at row 0, then render_frame draws the new prompt at row 1
    assert_buffer_eq(
        renderer.writer(),
        &[
            "Hello world", // User message in scrollback (overwrites initial prompt)
            ">",           // New prompt
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
    tool_call_with_id(name, &format!("call_{name}"), args)
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
    tool_result_with_id(name, &format!("call_{name}"), args, result)
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
    tx: &tokio::sync::mpsc::Sender<UserMessage>,
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
    tx: &tokio::sync::mpsc::Sender<UserMessage>,
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
