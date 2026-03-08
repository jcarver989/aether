use acp_utils::client::AcpPromptHandle;
use agent_client_protocol as acp;
use tui::testing::{TestTerminal, assert_buffer_eq};
use wisp::components::app::{App, AppAction, AppEffect, build_attachment_blocks};
use wisp::tui::{Line, Renderer as FrameRenderer, theme::Theme};

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
            screen: App::new(agent_name, config_options, vec![]),
            renderer: FrameRenderer::new(terminal, Theme::default()),
        }
    }

    fn writer(&self) -> &TestTerminal {
        self.renderer.writer()
    }

    fn update_render_context_with(&mut self, size: (u16, u16)) {
        self.renderer.update_render_context_with(size);
    }

    fn initial_render(&mut self) -> std::io::Result<()> {
        self.renderer.render(&mut self.screen)
    }

    async fn on_key_event(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        prompt_handle: &AcpPromptHandle,
        session_id: &acp::SessionId,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        let effects = self
            .screen
            .dispatch(AppAction::Key(key_event), self.renderer.context());
        self.apply_effects(effects, Some((prompt_handle, session_id)))
            .await
    }

    async fn on_session_update(&mut self, update: acp::SessionUpdate) -> std::io::Result<()> {
        let effects = self
            .screen
            .dispatch(AppAction::SessionUpdate(update), self.renderer.context());
        self.apply_effects_no_prompt(effects).await
    }

    async fn on_prompt_done(&mut self) -> std::io::Result<()> {
        let effects = self
            .screen
            .dispatch(AppAction::PromptDone, self.renderer.context());
        self.apply_effects_no_prompt(effects).await
    }

    async fn on_tick(&mut self) -> std::io::Result<()> {
        let effects = self
            .screen
            .dispatch(AppAction::Tick, self.renderer.context());
        self.apply_effects_no_prompt(effects).await
    }

    async fn on_paste(&mut self, text: &str) -> std::io::Result<()> {
        let effects = self
            .screen
            .dispatch(AppAction::Paste(text.to_string()), self.renderer.context());
        self.apply_effects_no_prompt(effects).await
    }

    async fn on_resize(&mut self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.renderer.update_render_context_with((cols, rows));
        let effects = self
            .screen
            .dispatch(AppAction::Resize { cols, rows }, self.renderer.context());
        self.apply_effects_no_prompt(effects).await
    }

    async fn set_file_picker_matches(
        &mut self,
        matches: Vec<wisp::components::file_picker::FileMatch>,
    ) -> std::io::Result<()> {
        let effects = self.screen.dispatch(
            AppAction::SetFilePickerMatches(matches),
            self.renderer.context(),
        );
        self.apply_effects_no_prompt(effects).await
    }

    async fn apply_effects(
        &mut self,
        effects: Vec<AppEffect>,
        prompt: Option<(&AcpPromptHandle, &acp::SessionId)>,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        let mut should_render = false;
        let mut action = LoopAction::Continue;

        for effect in effects {
            match effect {
                AppEffect::Exit => action = LoopAction::Exit,
                AppEffect::Render => should_render = true,
                AppEffect::PushScrollback(lines) => {
                    self.renderer.push_to_scrollback(&lines)?;
                }
                AppEffect::PromptSubmit {
                    user_input,
                    attachments,
                } => {
                    let Some((prompt_handle, session_id)) = prompt else {
                        return Err(std::io::Error::other("missing prompt context").into());
                    };
                    let outcome = build_attachment_blocks(&attachments).await;
                    if !outcome.warnings.is_empty() {
                        let warning_lines: Vec<Line> = outcome
                            .warnings
                            .into_iter()
                            .map(|warning| Line::new(format!("[wisp] {warning}")))
                            .collect();
                        self.renderer.push_to_scrollback(&warning_lines)?;
                    }
                    prompt_handle.prompt(
                        session_id,
                        &user_input,
                        if outcome.blocks.is_empty() {
                            None
                        } else {
                            Some(outcome.blocks)
                        },
                    )?;
                }
                AppEffect::SetConfigOption {
                    config_id,
                    new_value,
                } => {
                    let Some((prompt_handle, session_id)) = prompt else {
                        return Err(std::io::Error::other("missing prompt context").into());
                    };
                    let _ = prompt_handle.set_config_option(session_id, &config_id, &new_value);
                }
                AppEffect::Cancel => {
                    let Some((prompt_handle, session_id)) = prompt else {
                        return Err(std::io::Error::other("missing prompt context").into());
                    };
                    prompt_handle.cancel(session_id)?;
                }
                AppEffect::AuthenticateMcpServer { server_name } => {
                    let Some((prompt_handle, session_id)) = prompt else {
                        return Err(std::io::Error::other("missing prompt context").into());
                    };
                    let _ = prompt_handle.authenticate_mcp_server(session_id, &server_name);
                }
                AppEffect::AuthenticateProvider { method_id } => {
                    let Some((prompt_handle, session_id)) = prompt else {
                        return Err(std::io::Error::other("missing prompt context").into());
                    };
                    let _ = prompt_handle.authenticate(session_id, &method_id);
                }
                AppEffect::SetTheme { .. } => {
                    // Theme apply side effects are handled in terminal_ui; renderer tests ignore.
                }
            }
        }

        if should_render {
            self.renderer.render(&mut self.screen)?;
        }

        Ok(action)
    }

    #[allow(clippy::unused_async)]
    async fn apply_effects_no_prompt(&mut self, effects: Vec<AppEffect>) -> std::io::Result<()> {
        let mut should_render = false;

        for effect in effects {
            match effect {
                AppEffect::Exit => {}
                AppEffect::Render => should_render = true,
                AppEffect::PushScrollback(lines) => {
                    self.renderer.push_to_scrollback(&lines)?;
                }
                AppEffect::PromptSubmit { .. }
                | AppEffect::SetConfigOption { .. }
                | AppEffect::SetTheme { .. }
                | AppEffect::Cancel
                | AppEffect::AuthenticateMcpServer { .. }
                | AppEffect::AuthenticateProvider { .. } => {
                    panic!("unexpected prompt/config/cancel effect without prompt context");
                }
            }
        }

        if should_render {
            self.renderer.render(&mut self.screen)?;
        }

        Ok(())
    }
}

/// Test events that can be fed to the renderer.
enum TestEvent {
    Update(Box<acp::SessionUpdate>),
    PromptDone,
}

/// Build the expected bordered prompt lines for a given terminal width.
/// Returns `[top_border, input_line, bottom_border, status_line]`.
fn expected_prompt(width: u16, input: &str, agent_name: &str) -> Vec<String> {
    let w = width as usize;
    let inner = w - 2;
    let top = format!("╭{}╮", "─".repeat(inner));
    // Middle: │ > input + padding + │
    let prefix_len = 1 + 2 + input.len(); // space + "> " + input
    let pad = inner.saturating_sub(prefix_len);
    let middle = format!("│ > {}{:pad$}│", input, "");
    let bottom = format!("╰{}╯", "─".repeat(inner));
    let status = format!("  {agent_name}");
    vec![top, middle, bottom, status]
}

/// Build expected lines: scrollback lines + bordered prompt.
fn expected_with_prompt(
    scrollback: &[&str],
    width: u16,
    input: &str,
    agent_name: &str,
) -> Vec<String> {
    let mut lines: Vec<String> = scrollback.iter().map(ToString::to_string).collect();
    lines.extend(expected_prompt(width, input, agent_name));
    lines
}

fn has_file_picker(terminal: &TestTerminal) -> bool {
    let lines = terminal.get_lines();
    lines
        .iter()
        .any(|l| l.contains("▶ ") || l.contains("(no matches found)"))
}

fn has_command_picker(terminal: &TestTerminal) -> bool {
    let lines = terminal.get_lines();
    lines
        .iter()
        .any(|l| l.contains("Open configuration settings"))
}

fn has_config_menu(terminal: &TestTerminal) -> bool {
    let lines = terminal.get_lines();
    lines.iter().any(|l| l.contains("Configuration"))
}

fn has_config_picker(terminal: &TestTerminal) -> bool {
    let lines = terminal.get_lines();
    lines.iter().any(|l| l.contains("search:"))
}

fn config_menu_selected_label(terminal: &TestTerminal) -> Option<String> {
    let lines = terminal.get_lines();
    for line in &lines {
        if let Some(pos) = line.find("▶ ") {
            let rest = &line[pos + "▶ ".len()..];
            let label = rest.trim().to_string();
            if !label.is_empty() {
                return Some(label);
            }
        }
    }
    None
}

fn command_picker_visible_names(terminal: &TestTerminal) -> Vec<String> {
    let lines = terminal.get_lines();
    let mut names = Vec::new();
    for line in &lines {
        // Match lines like "▶ /name  description" or "  /name  description"
        let trimmed = line.trim();
        let content = if let Some(rest) = trimmed.strip_prefix("▶ ") {
            rest
        } else {
            trimmed
        };
        if let Some(rest) = content.strip_prefix('/') {
            if let Some(name) = rest.split_whitespace().next() {
                names.push(name.to_string());
            }
        }
    }
    names
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

    let expected = expected_with_prompt(&["│ Plan this"], TEST_WIDTH, "", TEST_AGENT);
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
        .await
        .unwrap();
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(" World"))),
        ))
        .await
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
        .await
        .unwrap();
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("Done"))),
        ))
        .await
        .unwrap();

    let expected = expected_with_prompt(&["│ Thinking", "", "Done"], TEST_WIDTH, "", TEST_AGENT);
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
        .await
        .unwrap();
    renderer
        .on_session_update(acp::SessionUpdate::AgentThoughtChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("B"))),
        ))
        .await
        .unwrap();
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("C"))),
        ))
        .await
        .unwrap();

    let expected = expected_with_prompt(&["A", "", "│ B", "", "C"], TEST_WIDTH, "", TEST_AGENT);
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
        &["│ Plan", "", "Answer", "", "│ Refine"],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_multiline_thought_prefixes_only_first_line() {
    let renderer = render(vec![thought_chunk("line one\nline two"), prompt_done()]).await;

    let expected = expected_with_prompt(&["│ line one", "│ line two"], TEST_WIDTH, "", TEST_AGENT);
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
            "│ Thinking",
            "",
            "⠋ search",
            "",
            "Done",
            "⠋ Working... (0/1 tools complete)",
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
        &["⠋ test_tool", "⠋ Working... (0/1 tools complete)"],
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
        &[r#"✓ test_tool {"arg1":"value1"}"#],
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
            r#"✓ search {"query":"test"}"#,
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
        &[r#"✓ Read {"file":"test.rs"}"#],
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
        &["⠋ Read", "⠋ Working... (0/1 tools complete)"],
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
        &["⠋ Read", "⠋ Working... (0/1 tools complete)"],
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
            r#"✓ Read {"file":"test.rs"}"#,
            r#"✓ Grep {"pattern":"foo"}"#,
            r#"✓ Glob {"path":"src/"}"#,
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
            r#"✓ Read {"file":"a.rs"}"#,
            "",
            "Done reading",
            "⠋ Write",
            "⠋ Working... (0/1 tools complete)",
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
            r#"✓ Read {"file":"a.rs"}"#,
            "",
            "Done reading",
            r#"✓ Write {"file":"b.rs"}"#,
        ],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

async fn render_with_size(events: Vec<TestEvent>, size: (u16, u16)) -> Renderer {
    let terminal = TestTerminal::new(size.0, size.1);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with(size);

    for event in events {
        match event {
            TestEvent::Update(update) => renderer.on_session_update(*update).await.unwrap(),
            TestEvent::PromptDone => renderer.on_prompt_done().await.unwrap(),
        }
    }

    renderer
}

async fn render(events: Vec<TestEvent>) -> Renderer {
    render_with_size(events, (TEST_WIDTH, 40)).await
}

#[tokio::test]
async fn test_user_message_submission() {
    let terminal = TestTerminal::new(TEST_WIDTH, 40);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((TEST_WIDTH, 40));

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    renderer.initial_render().unwrap();

    type_string(&mut renderer, "Hello world", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    // Simulate the agent finishing so the grid loader clears
    renderer.on_prompt_done().await.unwrap();

    let expected = expected_with_prompt(&["", "Hello world"], TEST_WIDTH, "", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

// ── Test helpers ──────────────────────────────────────────────────────

fn text_chunk(text: &str) -> TestEvent {
    TestEvent::Update(Box::new(acp::SessionUpdate::AgentMessageChunk(
        acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(text))),
    )))
}

fn thought_chunk(text: &str) -> TestEvent {
    TestEvent::Update(Box::new(acp::SessionUpdate::AgentThoughtChunk(
        acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(text))),
    )))
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
    TestEvent::Update(Box::new(acp::SessionUpdate::ToolCall(tc)))
}

fn tool_complete(id: &str) -> TestEvent {
    TestEvent::Update(Box::new(acp::SessionUpdate::ToolCallUpdate(
        acp::ToolCallUpdate::new(
            id.to_string(),
            acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::Completed),
        ),
    )))
}

fn tool_complete_with_display_meta(id: &str, display_meta: &serde_json::Value) -> TestEvent {
    let meta_map =
        serde_json::from_value::<serde_json::Map<String, serde_json::Value>>(serde_json::json!({
            "display": display_meta
        }))
        .unwrap();
    TestEvent::Update(Box::new(acp::SessionUpdate::ToolCallUpdate(
        acp::ToolCallUpdate::new(
            id.to_string(),
            acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::Completed),
        )
        .meta(meta_map),
    )))
}

fn tool_update_with_args(id: &str, args: &str) -> TestEvent {
    let value: serde_json::Value = serde_json::from_str(args).unwrap();
    TestEvent::Update(Box::new(acp::SessionUpdate::ToolCallUpdate(
        acp::ToolCallUpdate::new(
            id.to_string(),
            acp::ToolCallUpdateFields::new().raw_input(value),
        ),
    )))
}

async fn type_string(
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
            .await
            .unwrap();
    }
}

async fn press_enter(
    renderer: &mut Renderer,
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
        .await
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
        .await
        .unwrap();

    let expected = expected_with_prompt(
        &["⠋ Read", "⠋ Working... (0/1 tools complete)"],
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
        .await
        .unwrap();

    // Terminal resize triggers full re-render at new width
    renderer.on_resize(100, 30).await.unwrap();

    let expected = expected_with_prompt(
        &["⠋ Read", "⠋ Working... (0/1 tools complete)"],
        100,
        "",
        TEST_AGENT,
    );
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
        lines.iter().any(|l| l.contains('>')),
        "Prompt should be visible.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_prompt_done_does_not_duplicate_overflowed_lines() {
    let markers: Vec<String> = (1..=16).map(|i| format!("L{i:02}")).collect();
    let chunk = format!("```text\n{}\n```", markers.join("\n"));

    let renderer = render_with_size(vec![text_chunk(&chunk), prompt_done()], (40, 8)).await;

    let transcript = renderer.writer().get_transcript_lines();
    for marker in markers.iter().take(8) {
        let count = transcript
            .iter()
            .filter(|line| line.contains(marker))
            .count();
        assert_eq!(
            count,
            1,
            "Marker {marker} should appear exactly once in transcript, got {count}.\nTranscript:\n{}",
            transcript.join("\n")
        );
    }
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

    type_string(&mut renderer, "hello", &handle, &session_id).await;

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
    )
    .await;
    press_backspace(&mut renderer, &handle, &session_id).await;
    press_backspace(&mut renderer, &handle, &session_id).await;

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

    type_string(&mut renderer, "hello", &handle, &session_id).await;
    press_backspace(&mut renderer, &handle, &session_id).await;

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
        .await
        .unwrap();
    assert!(
        has_file_picker(renderer.writer()),
        "File picker should be open after typing @"
    );

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
        .await
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
        .await
        .unwrap();
    assert!(
        has_file_picker(renderer.writer()),
        "File picker should be open"
    );

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
        .await
        .unwrap();

    assert!(
        !has_file_picker(renderer.writer()),
        "File picker should be closed"
    );
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
                "OpenRouter / GPT-4o",
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
            .any(|l| l.contains("aether-acp") && l.contains("OpenRouter / GPT-4o")),
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
                "OpenRouter / GPT-4o",
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
                "Ollama / llama3",
            )],
        )
        .category(acp::SessionConfigOptionCategory::Model),
    ];

    renderer
        .on_session_update(acp::SessionUpdate::ConfigOptionUpdate(
            acp::ConfigOptionUpdate::new(new_config_options),
        ))
        .await
        .unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Ollama / llama3")),
        "Status line should show updated model.\nBuffer:\n{}",
        lines.join("\n")
    );
    assert!(
        !lines.iter().any(|l| l.contains("GPT-4o")),
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

    type_string(&mut renderer, "Hello", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

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

    type_string(&mut renderer, "Hello", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    // First session update should hide the loader
    renderer
        .on_session_update(acp::SessionUpdate::AgentMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new("Hi"))),
        ))
        .await
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

    type_string(&mut renderer, "Hello", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    renderer.on_prompt_done().await.unwrap();

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

    type_string(&mut renderer, "Hello", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    let lines_before: Vec<String> = renderer.writer().get_lines();

    renderer.on_tick().await.unwrap();

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

    renderer.on_tick().await.unwrap();

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

    renderer.on_paste("hello world").await.unwrap();

    let expected = expected_prompt(80, "hello world", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_paste_strips_control_characters() {
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    renderer.on_paste("line1\nline2\ttab").await.unwrap();

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
        .await
        .unwrap();
    assert!(
        has_file_picker(renderer.writer()),
        "File picker should be open"
    );

    // Paste should close the picker and append text
    renderer.on_paste("pasted text").await.unwrap();

    assert!(
        !has_file_picker(renderer.writer()),
        "File picker should be closed"
    );
    let expected = expected_prompt(80, "@pasted text", TEST_AGENT);
    assert_buffer_eq(renderer.writer(), &expected);
}

async fn send_key(
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
        .await
        .unwrap();
}

async fn open_picker_with_files(
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
    )
    .await;

    // Replace the picker with known entries via dispatch
    let matches: Vec<FileMatch> = files
        .into_iter()
        .map(|name| FileMatch {
            path: std::path::PathBuf::from(name),
            display_name: name.to_string(),
        })
        .collect();
    renderer.set_file_picker_matches(matches).await.unwrap();
}

fn assert_picker_renders_selected(terminal: &TestTerminal, expected_file: &str) {
    let lines = terminal.get_lines();
    let marker = format!("▶ {expected_file}");
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
    )
    .await;

    // Initially selected_index=0
    assert_picker_renders_selected(renderer.writer(), "alpha.rs");

    // Down arrow → beta.rs
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
    assert_picker_renders_selected(renderer.writer(), "beta.rs");

    // Down arrow → gamma.rs
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
    assert_picker_renders_selected(renderer.writer(), "gamma.rs");

    // Down arrow wraps → alpha.rs
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
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
    )
    .await;

    // Up from index 0 wraps → gamma.rs
    send_key(
        &mut renderer,
        KeyCode::Up,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
    assert_picker_renders_selected(renderer.writer(), "gamma.rs");

    // Up again → beta.rs
    send_key(
        &mut renderer,
        KeyCode::Up,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
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
    )
    .await;

    // Ctrl+N → beta.rs
    send_key(
        &mut renderer,
        KeyCode::Char('n'),
        KeyModifiers::CONTROL,
        &handle,
        &session_id,
    )
    .await;
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
    )
    .await;

    // Ctrl+P from index 0 wraps → beta.rs
    send_key(
        &mut renderer,
        KeyCode::Char('p'),
        KeyModifiers::CONTROL,
        &handle,
        &session_id,
    )
    .await;
    assert_picker_renders_selected(renderer.writer(), "beta.rs");
}

async fn press_backspace(
    renderer: &mut Renderer,
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
        .await
        .unwrap();
}

// ── Config menu tests ────────────────────────────────────────────────

fn make_config_options() -> Vec<acp::SessionConfigOption> {
    vec![
        acp::SessionConfigOption::select(
            "model".to_string(),
            "Model".to_string(),
            "openrouter:openai/gpt-4o".to_string(),
            vec![
                acp::SessionConfigSelectOption::new(
                    "openrouter:openai/gpt-4o".to_string(),
                    "OpenRouter / GPT-4o".to_string(),
                ),
                acp::SessionConfigSelectOption::new(
                    "anthropic:claude-sonnet-4-5".to_string(),
                    "Anthropic / Claude Sonnet 4.5".to_string(),
                ),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model),
    ]
}

#[tokio::test]
async fn test_config_command_opens_menu_for_single_option() {
    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    // Config menu should open; picker requires explicit Enter
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );
    assert!(
        !has_config_picker(renderer.writer()),
        "Config picker should not be visible"
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

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );
    assert!(
        !has_config_picker(renderer.writer()),
        "Config picker should not be visible"
    );

    // Open the picker by pressing Enter on the selected menu entry
    press_enter(&mut renderer, &handle, &session_id).await;
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );
    assert!(
        has_config_picker(renderer.writer()),
        "Config picker should be visible"
    );

    // First ESC closes the picker
    send_key(
        &mut renderer,
        KeyCode::Esc,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );
    assert!(
        !has_config_picker(renderer.writer()),
        "Config picker should not be visible"
    );

    // Second ESC closes the menu
    send_key(
        &mut renderer,
        KeyCode::Esc,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
    assert!(
        !has_config_menu(renderer.writer()),
        "Config menu should not be visible"
    );
}

#[tokio::test]
async fn test_config_menu_arrow_navigation_single_entry() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    // With single config option + Theme + MCP servers, menu has 3 entries: Model, Theme, MCP Servers
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );
    assert!(
        !has_config_picker(renderer.writer()),
        "Config picker should not be visible"
    );
    let label = config_menu_selected_label(renderer.writer());
    assert!(
        label.as_deref().is_some_and(|l| l.contains("Model")),
        "Initial selection should be Model, got: {label:?}"
    );

    // Down goes to Theme (index 1)
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
    let label = config_menu_selected_label(renderer.writer());
    assert!(
        label.as_deref().is_some_and(|l| l.contains("Theme")),
        "Second selection should be Theme, got: {label:?}"
    );

    // Down again goes to MCP Servers (index 2)
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
    let label = config_menu_selected_label(renderer.writer());
    assert!(
        label.as_deref().is_some_and(|l| l.contains("MCP Servers")),
        "Third selection should be MCP Servers, got: {label:?}"
    );

    // Down again wraps back to Model (index 0)
    send_key(
        &mut renderer,
        KeyCode::Down,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;
    let label = config_menu_selected_label(renderer.writer());
    assert!(
        label.as_deref().is_some_and(|l| l.contains("Model")),
        "Wrapped selection should be Model, got: {label:?}"
    );
}

#[tokio::test]
async fn test_config_single_option_shows_model_picker() {
    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    // Menu opens; press Enter to open the model picker
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );
    press_enter(&mut renderer, &handle, &session_id).await;

    assert!(
        has_config_picker(renderer.writer()),
        "Config picker should be visible"
    );
    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Model search")),
        "Should show model overlay directly.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_config_picker_focuses_cursor_on_overlay_query() {
    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;
    // Open the picker from the menu
    press_enter(&mut renderer, &handle, &session_id).await;

    let lines = renderer.writer().get_lines();
    #[allow(clippy::cast_possible_truncation)]
    let search_row = lines
        .iter()
        .position(|l| l.contains("Model search:"))
        .expect("model search header row should be rendered") as u16;
    let (cursor_col, cursor_row) = renderer.writer().cursor_position();

    assert_eq!(
        cursor_row,
        search_row,
        "Cursor should be on overlay search row.\nBuffer:\n{}",
        lines.join("\n")
    );
    // Overlay border "│ " (2 cols) + "  Model search: " (16 cols) = 18
    assert_eq!(cursor_col, 18);
}

#[tokio::test]
async fn test_config_picker_filters_model_options() {
    let config_options = make_config_options();
    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;
    // Open the picker from the menu
    press_enter(&mut renderer, &handle, &session_id).await;

    type_string(&mut renderer, "claude", &handle, &session_id).await;

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

    let config_options = vec![
        acp::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![acp::SessionConfigSelectOption::new("m1", "M1")],
        ),
        acp::SessionConfigOption::select(
            "theme",
            "Theme",
            "dark",
            vec![acp::SessionConfigSelectOption::new("dark", "Dark")],
        ),
    ];

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );

    // Typing a character should not modify input buffer
    send_key(
        &mut renderer,
        KeyCode::Char('x'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;

    // Menu should still be open
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );
    // Input prompt is not rendered while overlay is open, so 'x' shouldn't appear anywhere
    let lines = renderer.writer().get_lines();
    assert!(
        !lines.iter().any(|l| l.contains('x')),
        "Typed char should be swallowed while config overlay is open.\nBuffer:\n{}",
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

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );

    // Ctrl+C should still exit even with menu open
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
        .await
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

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );

    // Simulate the agent responding with updated config
    let new_config = vec![
        acp::SessionConfigOption::select(
            "model".to_string(),
            "Model".to_string(),
            "anthropic:claude-sonnet-4-5".to_string(),
            vec![
                acp::SessionConfigSelectOption::new(
                    "openrouter:openai/gpt-4o".to_string(),
                    "OpenRouter / GPT-4o".to_string(),
                ),
                acp::SessionConfigSelectOption::new(
                    "anthropic:claude-sonnet-4-5".to_string(),
                    "Anthropic / Claude Sonnet 4.5".to_string(),
                ),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model),
    ];

    renderer
        .on_session_update(acp::SessionUpdate::ConfigOptionUpdate(
            acp::ConfigOptionUpdate::new(new_config),
        ))
        .await
        .unwrap();

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Claude Sonnet")),
        "Menu should reflect updated config.\nBuffer:\n{}",
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

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

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

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );
    let lines = renderer.writer().get_lines();
    // Even with no config options, the MCP Servers entry is always present
    assert!(
        lines.iter().any(|l| l.contains("MCP Servers")),
        "Should show MCP Servers entry even when no config options.\nBuffer:\n{}",
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
    )
    .await;

    assert!(
        has_command_picker(renderer.writer()),
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

    type_string(&mut renderer, "hello/", &handle, &session_id).await;

    assert!(
        !has_command_picker(renderer.writer()),
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
    )
    .await;
    assert!(
        has_command_picker(renderer.writer()),
        "Command picker should be open"
    );

    send_key(
        &mut renderer,
        KeyCode::Esc,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;

    assert!(
        !has_command_picker(renderer.writer()),
        "Esc should close command picker"
    );
    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains('/')),
        "Input buffer should retain '/' after Esc (matches file picker behavior).\nBuffer:\n{}",
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
    )
    .await;
    assert!(
        has_command_picker(renderer.writer()),
        "Command picker should be open"
    );

    send_key(
        &mut renderer,
        KeyCode::Backspace,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;

    assert!(
        !has_command_picker(renderer.writer()),
        "Backspace on empty query should close command picker"
    );
}

#[tokio::test]
async fn test_available_commands_update_stored() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    renderer
        .on_session_update(acp::SessionUpdate::AvailableCommandsUpdate(
            acp::AvailableCommandsUpdate::new(vec![
                acp::AvailableCommand::new("search", "Search code"),
                acp::AvailableCommand::new("web", "Browse the web"),
            ]),
        ))
        .await
        .unwrap();

    // Open command picker and verify commands appear in rendered output
    send_key(
        &mut renderer,
        KeyCode::Char('/'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;

    let names = command_picker_visible_names(renderer.writer());
    assert!(
        names.iter().any(|n| n == "search"),
        "Picker should show 'search' command. Got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "web"),
        "Picker should show 'web' command. Got: {names:?}"
    );
}

#[tokio::test]
async fn test_available_commands_update_extracts_hint() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let terminal = TestTerminal::new(80, 24);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[]);
    renderer.update_render_context_with((80, 24));
    renderer.initial_render().unwrap();

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

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
        .await
        .unwrap();

    // Open command picker and verify the hint appears in rendered output
    send_key(
        &mut renderer,
        KeyCode::Char('/'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;

    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("query pattern")),
        "Hint text should appear in command picker.\nBuffer:\n{}",
        lines.join("\n")
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
        .await
        .unwrap();

    // Open picker
    send_key(
        &mut renderer,
        KeyCode::Char('/'),
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;

    let names = command_picker_visible_names(renderer.writer());
    assert!(
        names.iter().any(|n| n == "config"),
        "Picker should include built-in config command. Got: {names:?}",
    );
    assert!(
        names.iter().any(|n| n == "search"),
        "Picker should include MCP search command. Got: {names:?}",
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
    )
    .await;
    assert!(
        has_command_picker(renderer.writer()),
        "Command picker should be open"
    );

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
        .await
        .unwrap();

    assert!(matches!(action, LoopAction::Exit));
}

// ── Display meta tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_tool_complete_with_display_meta_shows_display_value() {
    let renderer = render(vec![
        tool_call_with_id(
            "read_file",
            "call_1",
            r#"{"filePath":"/Users/josh/code/aether/Cargo.toml"}"#,
        ),
        tool_complete_with_display_meta(
            "call_1",
            &serde_json::json!({
                "title": "Read file",
                "value": "Cargo.toml, 156 lines"
            }),
        ),
    ])
    .await;

    let expected = expected_with_prompt(
        &["✓ Read file (Cargo.toml, 156 lines)"],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_tool_complete_without_display_meta_shows_raw_args() {
    let args = r#"{"filePath":"/Users/josh/code/aether/Cargo.toml"}"#;
    let renderer = render(vec![
        tool_call_with_id("read_file", "call_1", args),
        tool_complete("call_1"),
    ])
    .await;

    let expected = expected_with_prompt(
        &[r#"✓ read_file {"filePath":"/Users/josh/code/aether/Cargo.toml"}"#],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_running_tool_hides_raw_args() {
    let renderer = render(vec![tool_call_with_id(
        "read_file",
        "call_1",
        r#"{"filePath":"Cargo.toml"}"#,
    )])
    .await;

    let lines = renderer.writer().get_lines();
    let tool_line = lines.iter().find(|l| l.contains("read_file")).unwrap();
    assert!(
        !tool_line.contains("filePath"),
        "Running tool should hide raw args: {tool_line}"
    );
    assert_eq!(
        tool_line.trim(),
        "⠋ read_file",
        "Running tool should show only name: {tool_line}"
    );
}

#[tokio::test]
async fn test_display_meta_title_overrides_tool_name() {
    let renderer = render(vec![
        tool_call_with_id("coding__read_file", "call_1", r#"{"filePath":"main.rs"}"#),
        tool_complete_with_display_meta(
            "call_1",
            &serde_json::json!({
                "title": "Read file",
                "value": "main.rs, 42 lines"
            }),
        ),
    ])
    .await;

    let lines = renderer.writer().get_lines();
    let tool_line = lines.iter().find(|l| l.contains("✓")).unwrap();
    assert!(
        tool_line.contains("Read file"),
        "Display title should override raw tool name: {tool_line}"
    );
    assert!(
        tool_line.contains("(main.rs, 42 lines)"),
        "Display value should appear in parens: {tool_line}"
    );
}

#[tokio::test]
async fn test_multiple_tools_with_mixed_display_meta() {
    let renderer = render(vec![
        tool_call_with_id("read_file", "call_1", r#"{"filePath":"Cargo.toml"}"#),
        tool_call_with_id("external_tool", "call_2", r#"{"key":"value"}"#),
        tool_complete_with_display_meta(
            "call_1",
            &serde_json::json!({
                "title": "Read file",
                "value": "Cargo.toml, 156 lines"
            }),
        ),
        tool_complete("call_2"),
    ])
    .await;

    let expected = expected_with_prompt(
        &[
            "✓ Read file (Cargo.toml, 156 lines)",
            r#"✓ external_tool {"key":"value"}"#,
        ],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_command_display_meta_shows_exit_code() {
    let renderer = render(vec![
        tool_call_with_id("bash", "call_1", r#"{"command":"cargo test"}"#),
        tool_complete_with_display_meta(
            "call_1",
            &serde_json::json!({
                "title": "Run command",
                "value": "cargo test (exit 0)"
            }),
        ),
    ])
    .await;

    let expected = expected_with_prompt(
        &["✓ Run command (cargo test (exit 0))"],
        TEST_WIDTH,
        "",
        TEST_AGENT,
    );
    assert_buffer_eq(renderer.writer(), &expected);
}

#[tokio::test]
async fn test_config_overlay_renders_after_large_overflow_scrollback() {
    let config_options = make_config_options();
    // Small viewport to force overflow quickly
    let terminal = TestTerminal::new(40, 8);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((40, 8));
    renderer.initial_render().unwrap();

    // Feed a LOT of content in a single streaming response (no prompt_done)
    // This causes progressive flush to build up flushed_visual_count
    for i in 0..50 {
        let chunk = format!("Line {i:02} with enough content to wrap in 40 cols");
        renderer
            .on_session_update(acp::SessionUpdate::AgentMessageChunk(
                acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(&chunk))),
            ))
            .await
            .unwrap();
    }

    // Now open config overlay WHILE still in the streaming context
    // This is where the bug manifests - flushed_visual_count is high
    // but the overlay produces fewer lines
    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    // Assert overlay state is correct
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be open"
    );

    // Assert overlay content is actually visible
    let lines = renderer.writer().get_lines();
    assert!(
        lines.iter().any(|l| l.contains("Configuration")),
        "Configuration header should be visible in overlay.\nBuffer:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|l| l.contains("Model")),
        "Model config option should be visible in overlay.\nBuffer:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn test_config_overlay_open_close_after_overflow_keeps_prompt_and_layout_valid() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let config_options = make_config_options();
    let terminal = TestTerminal::new(40, 8);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &config_options);
    renderer.update_render_context_with((40, 8));
    renderer.initial_render().unwrap();

    // Create overflow history within a single streaming response
    for i in 0..50 {
        let chunk = format!("Line {i:02} with enough content to wrap in 40 cols");
        renderer
            .on_session_update(acp::SessionUpdate::AgentMessageChunk(
                acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(&chunk))),
            ))
            .await
            .unwrap();
    }

    let handle = AcpPromptHandle::noop();
    let session_id = acp::SessionId::new("test-session");

    // Open config overlay while flushed_visual_count is high
    type_string(&mut renderer, "/config", &handle, &session_id).await;
    press_enter(&mut renderer, &handle, &session_id).await;

    // Verify overlay rendered correctly
    assert!(
        has_config_menu(renderer.writer()),
        "Config menu should be visible"
    );
    let lines_before = renderer.writer().get_lines();
    assert!(
        lines_before.iter().any(|l| l.contains("Configuration")),
        "Configuration should be visible before closing.\nBuffer:\n{}",
        lines_before.join("\n")
    );

    // Close overlay with Esc
    send_key(
        &mut renderer,
        KeyCode::Esc,
        KeyModifiers::empty(),
        &handle,
        &session_id,
    )
    .await;

    // Verify normal prompt rendering resumes
    assert!(
        !has_config_menu(renderer.writer()),
        "Config menu should not be visible"
    );
    let lines_after = renderer.writer().get_lines();

    // Prompt border/status line should be visible
    assert!(
        lines_after
            .iter()
            .any(|l| l.contains('╭') || l.contains('╰')),
        "Prompt border should be visible after closing overlay.\nBuffer:\n{}",
        lines_after.join("\n")
    );

    // Should not have an empty managed frame (at least some content should render)
    let has_content = lines_after.iter().any(|l| !l.trim().is_empty());
    assert!(
        has_content,
        "Frame should not be empty after closing overlay.\nBuffer:\n{}",
        lines_after.join("\n")
    );
}
