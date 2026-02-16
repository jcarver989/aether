mod test_terminal;

use agent_client_protocol as acp;
use test_terminal::{TestTerminal, assert_buffer_eq};
use wisp::acp_connection::AcpPromptHandle;
use wisp::renderer::Renderer;

/// Test events that can be fed to the renderer.
enum TestEvent {
    Update(acp::SessionUpdate),
    PromptDone,
}

#[tokio::test]
async fn test_agent_message_text_chunks() {
    let renderer = render(vec![
        text_chunk("Hello"),
        text_chunk(" World"),
        prompt_done(),
    ])
    .await;

    assert_buffer_eq(renderer.writer(), &["Hello World", ">"]);
}

#[tokio::test]
async fn test_agent_message_tool_call() {
    let renderer = render(vec![tool_call("test_tool", r#"{"arg1": "value1"}"#)]).await;

    assert_buffer_eq(
        renderer.writer(),
        &[r#"● test_tool {"arg1":"value1"}"#, ">"],
    );
}

#[tokio::test]
async fn test_agent_message_tool_result() {
    let args = r#"{"arg1": "value1"}"#;
    let renderer = render(vec![
        tool_call("test_tool", args),
        tool_complete("call_test_tool"),
    ])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &[r#"● test_tool ✓ {"arg1":"value1"}"#, ">"],
    );
}

#[tokio::test]
async fn test_multiple_messages_sequence() {
    let args = r#"{"query": "test"}"#;
    let renderer = render(vec![
        text_chunk("Processing your request"),
        prompt_done(),
        tool_call("search", args),
        tool_complete("call_search"),
        text_chunk("Found results"),
        prompt_done(),
    ])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &[
            "Processing your request",
            r#"● search ✓ {"query":"test"}"#,
            "Found results",
            ">",
        ],
    );
}

#[tokio::test]
async fn test_streaming_tool_call_arguments() {
    // In ACP, streaming args come via ToolCallUpdate with raw_input
    let renderer = render(vec![
        tool_call_with_id("Read", "call_1", ""),
        tool_update_with_args("call_1", r#"{"file":"test.rs"}"#),
        tool_complete("call_1"),
    ])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &[r#"● Read ✓ {"file":"test.rs"}"#, ">"],
    );
}

#[tokio::test]
async fn test_in_progress_tool_call_updates_from_duplicate_requests() {
    // Repeated ToolCall messages with same ID update the entry, preserving name
    let renderer = render(vec![
        tool_call_with_id("Read", "call_1", ""),
        tool_call_with_id("", "call_1", r#"{"file":"test.rs"}"#),
    ])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &[r#"● Read {"file":"test.rs"}"#, ">"],
    );
}

#[tokio::test]
async fn test_tool_progress_renders_running_tool() {
    let renderer = render(vec![tool_call_with_id(
        "Read",
        "call_1",
        r#"{"file":"test.rs"}"#,
    )])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &[r#"● Read {"file":"test.rs"}"#, ">"],
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
        tool_complete("call_Read"),
        tool_complete("call_Grep"),
        tool_complete("call_Glob"),
    ])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &[
            r#"● Read ✓ {"file":"test.rs"}"#,
            r#"● Grep ✓ {"pattern":"foo"}"#,
            r#"● Glob ✓ {"path":"src/"}"#,
            ">",
        ],
    );
}

#[tokio::test]
async fn test_text_complete_preserves_running_tool_calls() {
    let renderer = render(vec![
        tool_call_with_id("Read", "call_1", r#"{"file": "a.rs"}"#),
        tool_call_with_id("Write", "call_2", r#"{"file": "b.rs"}"#),
        tool_complete("call_1"),
        text_chunk("Done reading"),
        prompt_done(),
    ])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &[
            r#"● Read ✓ {"file":"a.rs"}"#,
            "Done reading",
            r#"● Write {"file":"b.rs"}"#,
            ">",
        ],
    );
}

#[tokio::test]
async fn test_late_result_after_prompt_done() {
    let renderer = render(vec![
        tool_call_with_id("Read", "call_1", r#"{"file": "a.rs"}"#),
        tool_call_with_id("Write", "call_2", r#"{"file": "b.rs"}"#),
        tool_complete("call_1"),
        text_chunk("Done reading"),
        prompt_done(),
        tool_complete("call_2"),
    ])
    .await;

    assert_buffer_eq(
        renderer.writer(),
        &[
            r#"● Read ✓ {"file":"a.rs"}"#,
            "Done reading",
            r#"● Write ✓ {"file":"b.rs"}"#,
            ">",
        ],
    );
}

fn render_sync(events: Vec<TestEvent>, size: (u16, u16)) -> Renderer<TestTerminal> {
    let terminal = TestTerminal::new(size.0, size.1);
    let mut renderer = Renderer::new(terminal);
    renderer.update_render_context_with(size);

    for event in events {
        match event {
            TestEvent::Update(update) => renderer.on_session_update(update).unwrap(),
            TestEvent::PromptDone => renderer.on_prompt_done().unwrap(),
        }
    }

    renderer
}

async fn render(events: Vec<TestEvent>) -> Renderer<TestTerminal> {
    render_sync(events, (200, 40))
}

async fn render_with_size(events: Vec<TestEvent>, size: (u16, u16)) -> Renderer<TestTerminal> {
    render_sync(events, size)
}

#[tokio::test]
async fn test_user_message_submission() {
    let terminal = TestTerminal::new(200, 40);
    let mut renderer = Renderer::new(terminal);
    renderer.update_render_context_with((200, 40));

    let handle = AcpPromptHandle::disconnected();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "Hello world", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    assert_buffer_eq(
        renderer.writer(),
        &[
            "Hello world", // User message in scrollback
            ">",           // New prompt
        ],
    );
}

// ── Test helpers ──────────────────────────────────────────────────────

fn text_chunk(text: &str) -> TestEvent {
    TestEvent::Update(acp::SessionUpdate::AgentMessageChunk(
        acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(text))),
    ))
}

fn prompt_done() -> TestEvent {
    TestEvent::PromptDone
}

fn tool_call(name: &str, args: &str) -> TestEvent {
    tool_call_with_id(name, &format!("call_{name}"), args)
}

fn tool_call_with_id(name: &str, id: &str, args: &str) -> TestEvent {
    let mut tc = acp::ToolCall::new(id.to_string(), name);
    if !args.is_empty() {
        let value: serde_json::Value = serde_json::from_str(args)
            .unwrap_or_else(|_| serde_json::Value::String(args.to_string()));
        tc = tc.raw_input(value);
    }
    TestEvent::Update(acp::SessionUpdate::ToolCall(tc))
}

fn tool_complete(id: &str) -> TestEvent {
    TestEvent::Update(acp::SessionUpdate::ToolCallUpdate(
        acp::ToolCallUpdate::new(
            id.to_string(),
            acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::Completed),
        ),
    ))
}

fn tool_update_with_args(id: &str, args: &str) -> TestEvent {
    let value: serde_json::Value = serde_json::from_str(args).unwrap();
    TestEvent::Update(acp::SessionUpdate::ToolCallUpdate(
        acp::ToolCallUpdate::new(
            id.to_string(),
            acp::ToolCallUpdateFields::new().raw_input(value),
        ),
    ))
}

fn type_string<W: std::io::Write>(
    renderer: &mut Renderer<W>,
    text: &str,
    handle: &AcpPromptHandle,
    session_id: &acp::SessionId,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    for ch in text.chars() {
        let key_event = KeyEvent {
            code: KeyCode::Char(ch),
            modifiers: KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        };
        renderer
            .on_key_event(key_event, handle, session_id)
            .unwrap();
    }
}

fn press_enter<W: std::io::Write>(
    renderer: &mut Renderer<W>,
    handle: &AcpPromptHandle,
    session_id: &acp::SessionId,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let enter_event = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::empty(),
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    };
    renderer
        .on_key_event(enter_event, handle, session_id)
        .unwrap();
}

// ── Regression tests: small terminals that force scrolling ───────────

#[tokio::test]
async fn test_no_ghost_on_tool_completion_small_terminal() {
    let args = r#"{"file": "a.rs"}"#;
    let renderer = render_with_size(
        vec![
            tool_call("Read", args),
            tool_complete("call_Read"),
            text_chunk("Done"),
            prompt_done(),
        ],
        (80, 4),
    )
    .await;

    let lines = renderer.writer().get_lines();
    let tool_count = lines.iter().filter(|l| l.contains("Read")).count();
    assert_eq!(
        tool_count, 1,
        "Tool name should appear exactly once, got {tool_count}.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_tool_updates_in_place_after_scrollback_push() {
    let renderer = render_with_size(
        vec![
            tool_call_with_id("Read", "call_1", r#"{"file": "a.rs"}"#),
            tool_call_with_id("Write", "call_2", r#"{"file": "b.rs"}"#),
            tool_complete("call_1"),
            text_chunk("Halfway"),
            prompt_done(),
            tool_complete("call_2"),
        ],
        (80, 4),
    )
    .await;

    let lines = renderer.writer().get_lines();
    let write_count = lines.iter().filter(|l| l.contains("Write")).count();
    assert_eq!(
        write_count, 1,
        "Write tool should appear exactly once, got {write_count}.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_multiple_scrollback_pushes_tiny_terminal() {
    let renderer = render_with_size(
        vec![
            text_chunk("First message"),
            prompt_done(),
            text_chunk("Second message"),
            prompt_done(),
            text_chunk("Third message"),
            prompt_done(),
        ],
        (80, 3),
    )
    .await;

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains(">")),
        "Prompt should be visible.\nBuffer:\n{}",
        lines.join("\n")
    );
}
