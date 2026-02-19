mod test_terminal;

use acp_utils::client::AcpPromptHandle;
use agent_client_protocol as acp;
use test_terminal::{TestTerminal, assert_buffer_eq};
use wisp::components::app::{App, AppEvent};
use wisp::components::command_picker::CommandEntry;
use wisp::tui::Renderer as FrameRenderer;

const TEST_AGENT: &str = "test-agent";
const TEST_WIDTH: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopAction {
    Continue,
    Exit,
}

struct Renderer {
    screen: App,
    renderer: FrameRenderer<TestTerminal>,
}

impl Renderer {
    fn new(
        terminal: TestTerminal,
        agent_name: String,
        config_options: &[acp::SessionConfigOption],
    ) -> Self {
        Self {
            screen: App::new(agent_name, config_options),
            renderer: FrameRenderer::new(terminal),
        }
    }

    fn writer(&self) -> &TestTerminal {
        self.renderer.writer()
    }

    fn update_render_context_with(&mut self, size: (u16, u16)) {
        self.renderer.update_render_context_with(size);
    }

    fn initial_render(&mut self) -> std::io::Result<()> {
        self.renderer.render(&self.screen)
    }

    fn on_key_event(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        prompt_handle: &AcpPromptHandle,
        session_id: &acp::SessionId,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        let effects = self.screen.on_key_event(key_event);
        self.apply_effects(effects, Some((prompt_handle, session_id)))
    }

    fn on_session_update(&mut self, update: acp::SessionUpdate) -> std::io::Result<()> {
        let effects = self.screen.on_session_update(update);
        self.apply_effects_no_prompt(effects)
    }

    fn on_prompt_done(&mut self) -> std::io::Result<()> {
        let effects = self.screen.on_prompt_done(self.renderer.context().size);
        self.apply_effects_no_prompt(effects)
    }

    fn on_tick(&mut self) -> std::io::Result<()> {
        let effects = self.screen.on_tick();
        self.apply_effects_no_prompt(effects)
    }

    fn on_paste(&mut self, text: &str) -> std::io::Result<()> {
        let effects = self.screen.on_paste(text);
        self.apply_effects_no_prompt(effects)
    }

    fn on_resize(&mut self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.renderer.update_render_context_with((cols, rows));
        let effects = self.screen.on_resize(cols, rows);
        self.apply_effects_no_prompt(effects)
    }

    fn screen(&self) -> &App {
        &self.screen
    }

    fn screen_mut(&mut self) -> &mut App {
        &mut self.screen
    }

    fn available_commands(&self) -> &[CommandEntry] {
        self.screen.available_commands()
    }

    fn apply_effects(
        &mut self,
        effects: Vec<AppEvent>,
        prompt: Option<(&AcpPromptHandle, &acp::SessionId)>,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        let mut should_render = false;
        let mut action = LoopAction::Continue;

        for effect in effects {
            match effect {
                AppEvent::Exit => action = LoopAction::Exit,
                AppEvent::Render => should_render = true,
                AppEvent::PushScrollback(lines) => {
                    self.renderer.push_to_scrollback(&lines)?;
                }
                AppEvent::PromptSubmit {
                    user_input,
                    content_blocks,
                } => {
                    let Some((prompt_handle, session_id)) = prompt else {
                        return Err(std::io::Error::other("missing prompt context").into());
                    };
                    prompt_handle.prompt(session_id, &user_input, content_blocks)?;
                }
                AppEvent::SetConfigOption {
                    config_id,
                    new_value,
                } => {
                    let Some((prompt_handle, session_id)) = prompt else {
                        return Err(std::io::Error::other("missing prompt context").into());
                    };
                    let _ = prompt_handle.set_config_option(session_id, &config_id, &new_value);
                }
            }
        }

        if should_render {
            self.renderer.render(&self.screen)?;
        }

        Ok(action)
    }

    fn apply_effects_no_prompt(&mut self, effects: Vec<AppEvent>) -> std::io::Result<()> {
        let mut should_render = false;

        for effect in effects {
            match effect {
                AppEvent::Exit => {}
                AppEvent::Render => should_render = true,
                AppEvent::PushScrollback(lines) => {
                    self.renderer.push_to_scrollback(&lines)?;
                }
                AppEvent::PromptSubmit { .. } | AppEvent::SetConfigOption { .. } => {
                    panic!("unexpected prompt/config effect without prompt context");
                }
            }
        }

        if should_render {
            self.renderer.render(&self.screen)?;
        }

        Ok(())
    }
}

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
async fn test_agent_thought_chunks() {
    let renderer = render(vec![
        thought_chunk("Plan"),
        thought_chunk(" this"),
        prompt_done(),
    ])
    .await;

    let expected = expected_with_prompt(&["Thought: Plan this"], TEST_WIDTH, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_agent_message_chunks_stream_before_prompt_done() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("Hello"))),
        ))
        .unwrap();
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(" World"))),
        ))
        .unwrap();

    let expected = expected_with_prompt(&["Hello World"], TEST_WIDTH, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_thought_and_text_chunks_stream_before_prompt_done() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::AgentThoughtChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("Thinking"))),
        ))
        .unwrap();
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("Done"))),
        ))
        .unwrap();

    let expected = expected_with_prompt(
        &["Thought: Thinking", "", "Done"],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_text_and_thought_chunks_stream_in_arrival_order() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));
    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("A"))),
        ))
        .unwrap();
    renderer
        .on_session_update(acp::SessionUpdate::AgentThoughtChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("B"))),
        ))
        .unwrap();
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("C"))),
        ))
        .unwrap();

    let expected = expected_with_prompt(
        &["A", "", "Thought: B", "", "C"],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_thought_prefix_resets_after_non_thought_boundary() {
    let renderer = render(vec![
        thought_chunk("Plan"),
        text_chunk("Answer"),
        thought_chunk("Refine"),
        prompt_done(),
    ])
    .await;

    let expected = expected_with_prompt(
        &["Thought: Plan", "", "Answer", "", "Thought: Refine"],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_multiline_thought_prefixes_only_first_line() {
    let renderer = render(vec![thought_chunk("line one\nline two"), prompt_done()]).await;

    let expected = expected_with_prompt(
        &["Thought: line one", "line two"],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_tool_calls_interleave_with_thought_and_text_in_arrival_order() {
    let renderer = render(vec![
        thought_chunk("Thinking"),
        tool_call("search", r#"{"q":"rust"}"#),
        text_chunk("Done"),
    ])
    .await;

    let expected = expected_with_prompt(
        &[
            "Thought: Thinking",
            "",
            r#"⠋ search {"q":"rust"}"#,
            "",
            "Done",
        ],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_agent_message_tool_call() {
    let renderer = render(vec![tool_call("test_tool", r#"{"arg1": "value1"}"#)]).await;

    let expected = expected_with_prompt(
        &[r#"⠋ test_tool {"arg1":"value1"}"#],
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
            "",
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
        &[r#"⠋ Read {"file":"test.rs"}"#],
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
        &[r#"⠋ Read {"file":"test.rs"}"#],
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
            "",
            "Done reading",
            r#"⠋ Write {"file":"b.rs"}"#,
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
            "",
            "Done reading",
            r#"● Write ✓ {"file":"b.rs"}"#,
        ],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

fn render_sync(events: Vec<TestEvent>, size: (u16, u16)) -> Renderer {
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

async fn render(events: Vec<TestEvent>) -> Renderer {
    render_sync(events, (TEST_WIDTH, 40))
}

async fn render_with_size(events: Vec<TestEvent>, size: (u16, u16)) -> Renderer {
    render_sync(events, size)
}

#[tokio::test]
async fn test_user_message_submission() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "Hello world", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    // Simulate the agent finishing so the grid loader clears
    renderer.on_prompt_done().unwrap();

    let expected = expected_with_prompt(&["", "Hello world"], TEST_WIDTH, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

// ── Test helpers ──────────────────────────────────────────────────────

fn text_chunk(text: &str) -> TestEvent {
    TestEvent::Update(acp::SessionUpdate::AgentMessageChunk(
        acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(text))),
    ))
}

fn thought_chunk(text: &str) -> TestEvent {
    TestEvent::Update(acp::SessionUpdate::AgentThoughtChunk(
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

fn type_string(
    renderer: &mut Renderer,
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

fn press_enter(renderer: &mut Renderer, handle: &AcpPromptHandle, session_id: &acp::SessionId) {
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
        &[r#"⠋ Read {"file":"test.rs"}"#],
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

    let expected = expected_with_prompt(&[r#"⠋ Read {"file":"test.rs"}"#], 100, "", TEST_AGENT);
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
async fn test_wrapped_tool_update_does_not_duplicate_lines() {
    let long_args = r#"{"file":"src/some/really/long/path/that/forces/tool/status/wrapping.rs"}"#;
    let renderer = render_with_size(
        vec![
            tool_call_with_id("Read", "call_1", long_args),
            tool_complete("call_1"),
        ],
        (40, 12),
    )
    .await;

    let lines = renderer.writer().get_lines();
    let read_count = lines.iter().filter(|l| l.contains("Read")).count();
    assert_eq!(
        read_count,
        1,
        "Wrapped tool line should update in place, got {read_count} Read rows.\nBuffer:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|l| l.contains("✓")),
        "Completed status should be visible after wrapped update.\nBuffer:\n{}",
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

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "hello", &handle, &session_id);

    let expected = expected_prompt(80, "hello", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_wrapped_input_prompt_rerender_has_single_box() {
    let terminal = TestTerminal::new(32, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((32, 24));

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();
    type_string(
        &mut renderer,
        "this input prompt is long enough to wrap across multiple rows",
        &handle,
        &session_id,
    );
    press_backspace(&mut renderer, &handle, &session_id);
    press_backspace(&mut renderer, &handle, &session_id);

    let lines = renderer.writer().get_lines();
    let top_count = lines.iter().filter(|l| l.contains('╭')).count();
    let bottom_count = lines.iter().filter(|l| l.contains('╰')).count();
    let content_rows = lines.iter().filter(|l| l.starts_with('│')).count();

    assert_eq!(
        top_count,
        1,
        "Expected a single prompt top border after wrapped rerender.\nBuffer:\n{}",
        lines.join("\n")
    );
    assert_eq!(
        bottom_count,
        1,
        "Expected a single prompt bottom border after wrapped rerender.\nBuffer:\n{}",
        lines.join("\n")
    );
    assert!(
        content_rows >= 2,
        "Expected wrapped prompt content rows.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_backspace_updates_within_border() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "hello", &handle, &session_id);
    press_backspace(&mut renderer, &handle, &session_id);

    let expected = expected_prompt(80, "hell", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_ctrl_c_exits_while_file_picker_is_open() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    renderer
        .on_key_event(
            KeyEvent {
                code: KeyCode::Char('@'),
                modifiers: KeyModifiers::empty(),
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
            &handle,
            &session_id,
        )
        .unwrap();
    assert!(renderer.screen().has_file_picker());

    let action = renderer
        .on_key_event(
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
            &handle,
            &session_id,
        )
        .unwrap();

    assert!(matches!(action, LoopAction::Exit));
}

#[tokio::test]
async fn test_space_closes_file_picker_without_selection() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    renderer
        .on_key_event(
            KeyEvent {
                code: KeyCode::Char('@'),
                modifiers: KeyModifiers::empty(),
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
            &handle,
            &session_id,
        )
        .unwrap();
    assert!(renderer.screen().has_file_picker());

    renderer
        .on_key_event(
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::empty(),
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
            &handle,
            &session_id,
        )
        .unwrap();

    assert!(!renderer.screen().has_file_picker());
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

    let handle = AcpPromptHandle::noop();
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

    let handle = AcpPromptHandle::noop();
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

    let handle = AcpPromptHandle::noop();
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

    let handle = AcpPromptHandle::noop();
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

#[tokio::test]
async fn test_paste_inserts_all_text_at_once() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    renderer.on_paste("hello world").unwrap();

    let expected = expected_prompt(80, "hello world", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_paste_strips_control_characters() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    renderer.on_paste("line1\nline2\ttab").unwrap();

    let expected = expected_prompt(80, "line1line2tab", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_paste_closes_file_picker() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    // Open file picker with @
    renderer
        .on_key_event(
            KeyEvent {
                code: KeyCode::Char('@'),
                modifiers: KeyModifiers::empty(),
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
            &handle,
            &session_id,
        )
        .unwrap();
    assert!(renderer.screen().has_file_picker());

    // Paste should close the picker and append text
    renderer.on_paste("pasted text").unwrap();

    assert!(!renderer.screen().has_file_picker());
    let expected = expected_prompt(80, "@pasted text", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

fn send_key(
    renderer: &mut Renderer,
    code: crossterm::event::KeyCode,
    modifiers: crossterm::event::KeyModifiers,
    handle: &AcpPromptHandle,
    session_id: &acp::SessionId,
) {
    renderer
        .on_key_event(
            crossterm::event::KeyEvent {
                code,
                modifiers,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
            handle,
            session_id,
        )
        .unwrap();
}

fn open_picker_with_files(
    renderer: &mut Renderer,
    files: Vec<&str>,
    handle: &AcpPromptHandle,
    session_id: &acp::SessionId,
) {
    use wisp::components::file_picker::FileMatch;

    // Type @ to set input_buffer state correctly
    send_key(
        renderer,
        crossterm::event::KeyCode::Char('@'),
        crossterm::event::KeyModifiers::empty(),
        handle,
        session_id,
    );

    // Replace the picker with known entries
    let matches: Vec<FileMatch> = files
        .into_iter()
        .map(|name| FileMatch {
            path: std::path::PathBuf::from(name),
            display_name: name.to_string(),
        })
        .collect();
    renderer.screen_mut().open_file_picker_with_matches(matches);

    // Trigger re-render with the injected picker
    renderer.on_resize(80, 24).unwrap();
}

fn picker_selected_display_name(renderer: &Renderer) -> Option<String> {
    renderer.screen().file_picker_selected_display_name()
}

fn assert_picker_renders_selected(terminal: &TestTerminal, expected_file: &str) {
    let lines = terminal.get_lines();
    let marker = format!("▶ {}", expected_file);
    assert!(
        lines.iter().any(|l| l.contains(&marker)),
        "Expected '{}' to be selected in rendered output.\nBuffer:\n{}",
        expected_file,
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_file_picker_down_arrow_moves_selection() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    open_picker_with_files(
        &mut renderer,
        vec!["alpha.rs", "beta.rs", "gamma.rs"],
        &handle,
        &session_id,
    );

    // Initially selected_index=0
    assert_eq!(
        picker_selected_display_name(&renderer).as_deref(),
        Some("alpha.rs")
    );
    assert_picker_renders_selected(renderer.writer(), "alpha.rs");

    // Down arrow → beta.rs
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert_eq!(
        picker_selected_display_name(&renderer).as_deref(),
        Some("beta.rs")
    );
    assert_picker_renders_selected(renderer.writer(), "beta.rs");

    // Down arrow → gamma.rs
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert_eq!(
        picker_selected_display_name(&renderer).as_deref(),
        Some("gamma.rs")
    );
    assert_picker_renders_selected(renderer.writer(), "gamma.rs");

    // Down arrow wraps → alpha.rs
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert_eq!(
        picker_selected_display_name(&renderer).as_deref(),
        Some("alpha.rs")
    );
    assert_picker_renders_selected(renderer.writer(), "alpha.rs");
}

#[tokio::test]
async fn test_file_picker_up_arrow_moves_selection() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    open_picker_with_files(
        &mut renderer,
        vec!["alpha.rs", "beta.rs", "gamma.rs"],
        &handle,
        &session_id,
    );

    // Up from index 0 wraps → gamma.rs
    send_key(
        &mut renderer,
        KeyCode::Up,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert_eq!(
        picker_selected_display_name(&renderer).as_deref(),
        Some("gamma.rs")
    );
    assert_picker_renders_selected(renderer.writer(), "gamma.rs");

    // Up again → beta.rs
    send_key(
        &mut renderer,
        KeyCode::Up,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert_eq!(
        picker_selected_display_name(&renderer).as_deref(),
        Some("beta.rs")
    );
    assert_picker_renders_selected(renderer.writer(), "beta.rs");
}

#[tokio::test]
async fn test_file_picker_ctrl_n_moves_down() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    open_picker_with_files(
        &mut renderer,
        vec!["alpha.rs", "beta.rs"],
        &handle,
        &session_id,
    );

    // Ctrl+N → beta.rs
    send_key(
        &mut renderer,
        KeyCode::Char('n'),
        KeyModifiers::CONTROL,
        &handle,
        &session_id,
    );
    assert_eq!(
        picker_selected_display_name(&renderer).as_deref(),
        Some("beta.rs")
    );
    assert_picker_renders_selected(renderer.writer(), "beta.rs");
}

#[tokio::test]
async fn test_file_picker_ctrl_p_moves_up() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    open_picker_with_files(
        &mut renderer,
        vec!["alpha.rs", "beta.rs"],
        &handle,
        &session_id,
    );

    // Ctrl+P from index 0 wraps → beta.rs
    send_key(
        &mut renderer,
        KeyCode::Char('p'),
        KeyModifiers::CONTROL,
        &handle,
        &session_id,
    );
    assert_eq!(
        picker_selected_display_name(&renderer).as_deref(),
        Some("beta.rs")
    );
    assert_picker_renders_selected(renderer.writer(), "beta.rs");
}

fn press_backspace(renderer: &mut Renderer, handle: &AcpPromptHandle, session_id: &acp::SessionId) {
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

// ── Config menu tests ────────────────────────────────────────────────

fn make_config_options() -> Vec<acp::SessionConfigOption> {
    vec![
        acp::SessionConfigOption::select(
            "provider".to_string(),
            "Provider".to_string(),
            "openrouter".to_string(),
            vec![
                acp::SessionConfigSelectOption::new(
                    "openrouter".to_string(),
                    "OpenRouter".to_string(),
                ),
                acp::SessionConfigSelectOption::new(
                    "anthropic".to_string(),
                    "Anthropic".to_string(),
                ),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model),
        acp::SessionConfigOption::select(
            "model".to_string(),
            "Model".to_string(),
            "openrouter:openai/gpt-4o".to_string(),
            vec![
                acp::SessionConfigSelectOption::new(
                    "openrouter:openai/gpt-4o".to_string(),
                    "GPT-4o".to_string(),
                ),
                acp::SessionConfigSelectOption::new(
                    "openrouter:anthropic/claude-3.5-sonnet".to_string(),
                    "Claude Sonnet".to_string(),
                ),
            ],
        ),
    ]
}

#[tokio::test]
async fn test_config_command_opens_menu() {
    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    assert!(renderer.screen().has_config_menu());
    let lines = renderer.writer().get_lines();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("Provider") && l.contains("OpenRouter")),
        "Config menu should show Provider option.\nBuffer:\n{}",
        lines.join("\n")
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("Model") && l.contains("GPT-4o")),
        "Config menu should show Model option.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_config_menu_esc_closes() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);
    assert!(renderer.screen().has_config_menu());

    send_key(
        &mut renderer,
        KeyCode::Esc,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert!(!renderer.screen().has_config_menu());
}

#[tokio::test]
async fn test_config_menu_arrow_navigation() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    // Initially Provider is selected (index 0)
    assert_eq!(renderer.screen().config_menu_selected_index(), Some(0));

    // Down arrow → Model (index 1)
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert_eq!(renderer.screen().config_menu_selected_index(), Some(1));

    // Down wraps → Provider (index 0)
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert_eq!(renderer.screen().config_menu_selected_index(), Some(0));

    // Up wraps → Model (index 1)
    send_key(
        &mut renderer,
        KeyCode::Up,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert_eq!(renderer.screen().config_menu_selected_index(), Some(1));
}

#[tokio::test]
async fn test_config_menu_enter_opens_overlay_picker() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    // Enter opens picker for selected row (Provider)
    send_key(
        &mut renderer,
        KeyCode::Enter,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );

    assert!(renderer.screen().has_config_picker());
    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Provider search")),
        "Should show provider overlay after pressing enter.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_config_picker_focuses_cursor_on_overlay_query() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);
    send_key(
        &mut renderer,
        KeyCode::Enter,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );

    let lines = renderer.writer().get_lines();
    let search_row = lines
        .iter()
        .position(|l| l.contains("Provider search:"))
        .expect("provider search header row should be rendered") as u16;
    let (cursor_col, cursor_row) = renderer.writer().cursor_position();

    assert_eq!(
        cursor_row,
        search_row,
        "Cursor should be on overlay search row.\nBuffer:\n{}",
        lines.join("\n")
    );
    assert_eq!(cursor_col, "  Provider search: ".len() as u16);
}

#[tokio::test]
async fn test_config_picker_filters_model_options() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    // Move to model row, open model picker.
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    send_key(
        &mut renderer,
        KeyCode::Enter,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );

    type_string(&mut renderer, "claude", &handle, &session_id);

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Claude Sonnet")),
        "Should show fuzzy-matched model result.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_config_menu_swallows_other_keys() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);
    assert!(renderer.screen().has_config_menu());

    // Typing a character should not modify input buffer
    send_key(
        &mut renderer,
        KeyCode::Char('x'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );

    // Menu should still be open
    assert!(renderer.screen().has_config_menu());
    // Input buffer should be empty (was cleared when /config opened)
    let lines = renderer.writer().get_lines();
    // The input prompt should show empty (just "> ")
    assert!(
        lines.iter().any(|l| l.contains("> ") && !l.contains("x")),
        "Typed char should be swallowed while config menu is open.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_config_menu_ctrl_c_exits() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);
    assert!(renderer.screen().has_config_menu());

    let action = renderer
        .on_key_event(
            crossterm::event::KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
            &handle,
            &session_id,
        )
        .unwrap();

    assert!(matches!(action, LoopAction::Exit));
}

#[tokio::test]
async fn test_config_menu_updates_on_config_option_event() {
    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);
    assert!(renderer.screen().has_config_menu());

    // Simulate the agent responding with updated config
    let new_config = vec![
        acp::SessionConfigOption::select(
            "provider".to_string(),
            "Provider".to_string(),
            "openrouter".to_string(),
            vec![
                acp::SessionConfigSelectOption::new(
                    "openrouter".to_string(),
                    "OpenRouter".to_string(),
                ),
                acp::SessionConfigSelectOption::new(
                    "anthropic".to_string(),
                    "Anthropic".to_string(),
                ),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model),
        acp::SessionConfigOption::select(
            "model".to_string(),
            "Model".to_string(),
            "openrouter:anthropic/claude-3.5-sonnet".to_string(),
            vec![
                acp::SessionConfigSelectOption::new(
                    "openrouter:openai/gpt-4o".to_string(),
                    "GPT-4o".to_string(),
                ),
                acp::SessionConfigSelectOption::new(
                    "openrouter:anthropic/claude-3.5-sonnet".to_string(),
                    "Claude Sonnet".to_string(),
                ),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model),
    ];

    renderer
        .on_session_update(acp::SessionUpdate::ConfigOptionUpdate(
            acp::ConfigOptionUpdate::new(new_config),
        ))
        .unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Claude Sonnet")),
        "Menu should reflect updated config.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_provider_confirm_auto_opens_model_picker_on_update() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    // Open provider picker.
    send_key(
        &mut renderer,
        KeyCode::Enter,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert!(renderer.screen().has_config_picker());

    // Select Anthropic provider and confirm.
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    send_key(
        &mut renderer,
        KeyCode::Enter,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert!(!renderer.screen().has_config_picker());

    // Provider update should auto-open model picker.
    let updated = vec![
        acp::SessionConfigOption::select(
            "provider".to_string(),
            "Provider".to_string(),
            "anthropic".to_string(),
            vec![
                acp::SessionConfigSelectOption::new(
                    "openrouter".to_string(),
                    "OpenRouter".to_string(),
                ),
                acp::SessionConfigSelectOption::new(
                    "anthropic".to_string(),
                    "Anthropic".to_string(),
                ),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model),
        acp::SessionConfigOption::select(
            "model".to_string(),
            "Model".to_string(),
            "anthropic:claude-sonnet-4-5".to_string(),
            vec![
                acp::SessionConfigSelectOption::new(
                    "anthropic:claude-sonnet-4-5".to_string(),
                    "Claude Sonnet 4.5".to_string(),
                ),
                acp::SessionConfigSelectOption::new(
                    "anthropic:claude-opus-4-1".to_string(),
                    "Claude Opus 4.1".to_string(),
                ),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model),
    ];

    renderer
        .on_session_update(acp::SessionUpdate::ConfigOptionUpdate(
            acp::ConfigOptionUpdate::new(updated),
        ))
        .unwrap();

    assert_eq!(renderer.screen().config_picker_config_id(), Some("model"));
    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Model search")),
        "Model picker should auto-open after provider update.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_config_clears_input_buffer() {
    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    // Input buffer should be cleared
    let lines = renderer.writer().get_lines();
    // The prompt line should not contain "/config"
    assert!(
        !lines.iter().any(|l| l.contains("/config")),
        "Input buffer should be cleared after /config.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_config_with_no_options_shows_placeholder() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id);
    press_enter(&mut renderer, &handle, &session_id);

    assert!(renderer.screen().has_config_menu());
    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("no config options")),
        "Should show placeholder when no options.\nBuffer:\n{}",
        lines.join("\n")
    );
}

// ── Command picker tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_slash_opens_command_picker() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    send_key(
        &mut renderer,
        KeyCode::Char('/'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );

    assert!(
        renderer.screen().has_command_picker(),
        "Typing / on empty buffer should open command picker"
    );
}

#[tokio::test]
async fn test_slash_mid_input_no_picker() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "hello/", &handle, &session_id);

    assert!(
        !renderer.screen().has_command_picker(),
        "Typing / mid-input should not open command picker"
    );
}

#[tokio::test]
async fn test_command_picker_esc_clears() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    send_key(
        &mut renderer,
        KeyCode::Char('/'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert!(renderer.screen().has_command_picker());

    send_key(
        &mut renderer,
        KeyCode::Esc,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );

    assert!(
        !renderer.screen().has_command_picker(),
        "Esc should close command picker"
    );
    let lines = renderer.writer().get_lines();
    assert!(
        !lines.iter().any(|l| l.contains("/")),
        "Input buffer should be cleared after Esc.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_command_picker_backspace_empty_closes() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    send_key(
        &mut renderer,
        KeyCode::Char('/'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert!(renderer.screen().has_command_picker());

    send_key(
        &mut renderer,
        KeyCode::Backspace,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );

    assert!(
        !renderer.screen().has_command_picker(),
        "Backspace on empty query should close command picker"
    );
}

#[tokio::test]
async fn test_available_commands_update_stored() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::AvailableCommandsUpdate(
            acp::AvailableCommandsUpdate::new(vec![
                acp::AvailableCommand::new("search", "Search code"),
                acp::AvailableCommand::new("web", "Browse the web"),
            ]),
        ))
        .unwrap();

    assert_eq!(renderer.available_commands().len(), 2);
    assert_eq!(renderer.available_commands()[0].name, "search");
    assert_eq!(renderer.available_commands()[1].name, "web");
}

#[tokio::test]
async fn test_available_commands_update_extracts_hint() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    renderer
        .on_session_update(acp::SessionUpdate::AvailableCommandsUpdate(
            acp::AvailableCommandsUpdate::new(vec![
                acp::AvailableCommand::new("search", "Search code").input(
                    acp::AvailableCommandInput::Unstructured(acp::UnstructuredCommandInput::new(
                        "query pattern",
                    )),
                ),
                acp::AvailableCommand::new("config", "Open settings"),
            ]),
        ))
        .unwrap();

    assert_eq!(renderer.available_commands().len(), 2);
    assert_eq!(
        renderer.available_commands()[0].hint.as_deref(),
        Some("query pattern"),
        "Command with Unstructured input should have hint"
    );
    assert_eq!(
        renderer.available_commands()[1].hint,
        None,
        "Command without input should have no hint"
    );
}

#[tokio::test]
async fn test_command_picker_shows_mcp_commands() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    // Feed available commands
    renderer
        .on_session_update(acp::SessionUpdate::AvailableCommandsUpdate(
            acp::AvailableCommandsUpdate::new(vec![acp::AvailableCommand::new(
                "search",
                "Search code",
            )]),
        ))
        .unwrap();

    // Open picker
    send_key(
        &mut renderer,
        KeyCode::Char('/'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );

    let names = renderer.screen().command_picker_match_names();
    assert!(
        names.contains(&"config"),
        "Picker should include built-in config command. Got: {:?}",
        names
    );
    assert!(
        names.contains(&"search"),
        "Picker should include MCP search command. Got: {:?}",
        names
    );
}

#[tokio::test]
async fn test_command_picker_ctrl_c_exits() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    send_key(
        &mut renderer,
        KeyCode::Char('/'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    );
    assert!(renderer.screen().has_command_picker());

    let action = renderer
        .on_key_event(
            crossterm::event::KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            },
            &handle,
            &session_id,
        )
        .unwrap();

    assert!(matches!(action, LoopAction::Exit));
}
