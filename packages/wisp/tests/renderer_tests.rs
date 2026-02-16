mod test_terminal;

use agent_client_protocol as acp;
use test_terminal::{TestTerminal, assert_buffer_eq};
use acp_utils::client::AcpPromptHandle;
use wisp::renderer::Renderer;

const TEST_AGENT: &str = "test-agent";
const TEST_WIDTH: u16 = 200;

/// Test events that can be fed to the renderer.
enum TestEvent {
    Update(acp::SessionUpdate),
    PromptDone,
}

/// Build the expected bordered prompt lines for a given terminal width.
/// Returns [top_border, input_line, bottom_border, status_line].
fn expected_prompt(width: u16, input: &str, agent_name: &str) -> Vec<String> {
    let w = width as usize;
    let inner = w - 2;
    let top = format!("╭{}╮", "─".repeat(inner));
    // Middle: │ > input + padding + │
    let prefix_len = 1 + 2 + input.len(); // space + "> " + input
    let pad = inner.saturating_sub(prefix_len);
    let middle = format!("│ > {}{:pad$}│", input, "");
    let bottom = format!("╰{}╯", "─".repeat(inner));
    let status = format!("  {}", agent_name);
    vec![top, middle, bottom, status]
}

/// Build expected lines: scrollback lines + bordered prompt.
fn expected_with_prompt(
    scrollback: &[&str],
    width: u16,
    input: &str,
    agent_name: &str,
) -> Vec<String> {
    let mut lines: Vec<String> = scrollback.iter().map(|s| s.to_string()).collect();
    lines.extend(expected_prompt(width, input, agent_name));
    lines
}

#[tokio::test]
async fn test_agent_message_text_chunks() {
    let renderer = render(vec![
        text_chunk("Hello"),
        text_chunk(" World"),
        prompt_done(),
    ])
    .await;

    let expected = expected_with_prompt(&["Hello World"], TEST_WIDTH, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_agent_message_tool_call() {
    let renderer = render(vec![tool_call("test_tool", r#"{"arg1": "value1"}"#)]).await;

    let expected = expected_with_prompt(
        &[r#"● test_tool {"arg1":"value1"}"#],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_agent_message_tool_result() {
    let args = r#"{"arg1": "value1"}"#;
    let renderer = render(vec![
        tool_call("test_tool", args),
        tool_complete("call_test_tool"),
    ])
    .await;

    let expected = expected_with_prompt(
        &[r#"● test_tool ✓ {"arg1":"value1"}"#],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
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

    let expected = expected_with_prompt(
        &[
            "Processing your request",
            r#"● search ✓ {"query":"test"}"#,
            "Found results",
        ],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_streaming_tool_call_arguments() {
    let renderer = render(vec![
        tool_call_with_id("Read", "call_1", ""),
        tool_update_with_args("call_1", r#"{"file":"test.rs"}"#),
        tool_complete("call_1"),
    ])
    .await;

    let expected = expected_with_prompt(
        &[r#"● Read ✓ {"file":"test.rs"}"#],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_in_progress_tool_call_updates_from_duplicate_requests() {
    let renderer = render(vec![
        tool_call_with_id("Read", "call_1", ""),
        tool_call_with_id("", "call_1", r#"{"file":"test.rs"}"#),
    ])
    .await;

    let expected = expected_with_prompt(
        &[r#"● Read {"file":"test.rs"}"#],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_tool_progress_renders_running_tool() {
    let renderer = render(vec![tool_call_with_id(
        "Read",
        "call_1",
        r#"{"file":"test.rs"}"#,
    )])
    .await;

    let expected = expected_with_prompt(
        &[r#"● Read {"file":"test.rs"}"#],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
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

    let expected = expected_with_prompt(
        &[
            r#"● Read ✓ {"file":"test.rs"}"#,
            r#"● Grep ✓ {"pattern":"foo"}"#,
            r#"● Glob ✓ {"path":"src/"}"#,
        ],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
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

    let expected = expected_with_prompt(
        &[
            r#"● Read ✓ {"file":"a.rs"}"#,
            "Done reading",
            r#"● Write {"file":"b.rs"}"#,
        ],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
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

    let expected = expected_with_prompt(
        &[
            r#"● Read ✓ {"file":"a.rs"}"#,
            "Done reading",
            r#"● Write ✓ {"file":"b.rs"}"#,
        ],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

fn render_sync(events: Vec<TestEvent>, size: (u16, u16)) -> Renderer<TestTerminal> {
    let terminal = TestTerminal::new(size.0, size.1);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
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
    render_sync(events, (TEST_WIDTH, 40))
}

async fn render_with_size(events: Vec<TestEvent>, size: (u16, u16)) -> Renderer<TestTerminal> {
    render_sync(events, size)
}

#[tokio::test]
async fn test_user_message_submission() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));

    let handle = AcpPromptHandle::disconnected();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "Hello world", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    // Simulate the agent finishing so the grid loader clears
    renderer.on_prompt_done().unwrap();

    let expected = expected_with_prompt(&["Hello world"], TEST_WIDTH, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
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

// ── Regression: tool calls must render after initial_render ──────────

#[tokio::test]
async fn test_in_progress_tool_call_visible_after_initial_render() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));

    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::ToolCall(
            acp::ToolCall::new("call_1".to_string(), "Read")
                .raw_input(serde_json::json!({"file": "test.rs"})),
        ))
        .unwrap();

    let expected = expected_with_prompt(
        &[r#"● Read {"file":"test.rs"}"#],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_in_progress_tool_call_renders_correctly_after_resize() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::ToolCall(
            acp::ToolCall::new("call_1".to_string(), "Read")
                .raw_input(serde_json::json!({"file": "test.rs"})),
        ))
        .unwrap();

    // Terminal resize triggers full re-render at new width
    renderer.on_resize(100, 30).unwrap();

    let expected = expected_with_prompt(&[r#"● Read {"file":"test.rs"}"#], 100, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
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
        (80, 8),
    )
    .await;

    let lines = renderer.writer().get_lines();
    let tool_count = lines.iter().filter(|l| l.contains("Read")).count();
    assert_eq!(
        tool_count,
        1,
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
        (80, 10),
    )
    .await;

    let lines = renderer.writer().get_lines();
    let write_count = lines.iter().filter(|l| l.contains("Write")).count();
    assert_eq!(
        write_count,
        1,
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
        (80, 8),
    )
    .await;

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains(">")),
        "Prompt should be visible.\nBuffer:\n{}",
        lines.join("\n")
    );
}

// ── New tests: bordered input + status line ──────────────────────────

#[tokio::test]
async fn test_typing_renders_within_bordered_input() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));

    let handle = AcpPromptHandle::disconnected();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "hello", &handle, &session_id);

    let expected = expected_prompt(80, "hello", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_backspace_updates_within_border() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));

    let handle = AcpPromptHandle::disconnected();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "hello", &handle, &session_id);
    press_backspace(&mut renderer, &handle, &session_id);

    let expected = expected_prompt(80, "hell", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_status_line_shows_agent_name() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, "claude-code".to_string(), &[]);
    renderer.update_render_context_with((80, 24));

    renderer.initial_render().unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("claude-code")),
        "Status line should show agent name.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_status_line_shows_model_from_config_options() {
    let config_options = vec![
        acp::SessionConfigOption::select(
            "model",
            "Model",
            "openrouter:gpt-4o",
            vec![acp::SessionConfigSelectOption::new(
                "openrouter:gpt-4o",
                "gpt-4o",
            )],
        )
        .category(acp::SessionConfigOptionCategory::Model),
    ];

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, "aether-acp".to_string(), &config_options);
    renderer.update_render_context_with((80, 24));

    renderer.initial_render().unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("aether-acp") && l.contains("gpt-4o")),
        "Status line should show agent name and model.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_status_line_updates_on_config_option_update() {
    let config_options = vec![
        acp::SessionConfigOption::select(
            "model",
            "Model",
            "openrouter:gpt-4o",
            vec![acp::SessionConfigSelectOption::new(
                "openrouter:gpt-4o",
                "gpt-4o",
            )],
        )
        .category(acp::SessionConfigOptionCategory::Model),
    ];

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, "aether-acp".to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    // Send a ConfigOptionUpdate with a new model
    let new_config_options = vec![
        acp::SessionConfigOption::select(
            "model",
            "Model",
            "ollama:llama3",
            vec![acp::SessionConfigSelectOption::new(
                "ollama:llama3",
                "llama3",
            )],
        )
        .category(acp::SessionConfigOptionCategory::Model),
    ];

    renderer
        .on_session_update(acp::SessionUpdate::ConfigOptionUpdate(
            acp::ConfigOptionUpdate::new(new_config_options),
        ))
        .unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("llama3")),
        "Status line should show updated model.\nBuffer:\n{}",
        lines.join("\n")
    );
    assert!(
        !lines.iter().any(|l| l.contains("gpt-4o")),
        "Status line should no longer show old model.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_empty_prompt_renders_bordered_box() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));

    renderer.initial_render().unwrap();

    let expected = expected_prompt(80, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

// ── Grid loader tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_grid_loader_visible_after_prompt_submit() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));

    let handle = AcpPromptHandle::disconnected();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "Hello", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    let lines = renderer.writer().get_lines();
    let has_spinner = lines.iter().any(|l| l.contains('⠋'));
    assert!(
        has_spinner,
        "Spinner should be visible after prompt submit.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_grid_loader_disappears_on_session_update() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));

    let handle = AcpPromptHandle::disconnected();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "Hello", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    // First session update should hide the loader
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("Hi"))),
        ))
        .unwrap();

    let lines = renderer.writer().get_lines();
    let has_braille = lines
        .iter()
        .any(|l| "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".chars().any(|c| l.contains(c)));
    assert!(
        !has_braille,
        "Spinner should disappear after session update.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_grid_loader_disappears_on_prompt_done() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));

    let handle = AcpPromptHandle::disconnected();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "Hello", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    renderer.on_prompt_done().unwrap();

    let lines = renderer.writer().get_lines();
    let has_braille = lines
        .iter()
        .any(|l| "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".chars().any(|c| l.contains(c)));
    assert!(
        !has_braille,
        "Spinner should disappear after prompt done.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_grid_loader_not_visible_on_initial_render() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));

    renderer.initial_render().unwrap();

    let expected = expected_prompt(80, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_on_tick_advances_animation() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));

    let handle = AcpPromptHandle::disconnected();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "Hello", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    let lines_before: Vec<String> = renderer.writer().get_lines();

    renderer.on_tick().unwrap();

    let lines_after: Vec<String> = renderer.writer().get_lines();

    // The frames should differ because the animation advanced
    assert_ne!(
        lines_before, lines_after,
        "on_tick should advance the animation and produce a different frame"
    );
}

#[tokio::test]
async fn test_on_tick_noop_when_not_waiting() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));

    renderer.initial_render().unwrap();

    let lines_before: Vec<String> = renderer.writer().get_lines();

    renderer.on_tick().unwrap();

    let lines_after: Vec<String> = renderer.writer().get_lines();

    assert_eq!(
        lines_before, lines_after,
        "on_tick should be a no-op when not waiting for response"
    );
}

fn press_backspace<W: std::io::Write>(
    renderer: &mut Renderer<W>,
    handle: &AcpPromptHandle,
    session_id: &acp::SessionId,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let backspace_event = KeyEvent {
        code: KeyCode::Backspace,
        modifiers: KeyModifiers::empty(),
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    };
    renderer
        .on_key_event(backspace_event, handle, session_id)
        .unwrap();
}
