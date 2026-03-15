use acp_utils::client::AcpEvent;
use acp_utils::client::AcpPromptHandle;
use agent_client_protocol as acp;
use tui::Theme;
use tui::advanced::Renderer as FrameRenderer;
use tui::advanced::RendererCommand;
use tui::testing::TestTerminal;
use tui::{Component, Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use wisp::components::app::App;

pub(super) const TEST_AGENT: &str = "test-agent";
pub(super) const TEST_WIDTH: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoopAction {
    Continue,
    Exit,
}

pub(super) struct Renderer {
    app: App,
    frame_renderer: FrameRenderer<TestTerminal>,
}

impl Renderer {
    pub(super) fn new(
        terminal: TestTerminal,
        agent_name: String,
        config_options: &[acp::SessionConfigOption],
        size: (u16, u16),
    ) -> Self {
        let app = App::new(
            acp::SessionId::new("test"),
            agent_name,
            config_options,
            vec![],
            std::path::PathBuf::from("."),
            AcpPromptHandle::noop(),
        );
        let frame_renderer = FrameRenderer::new(terminal, Theme::default(), size);
        Self {
            app,
            frame_renderer,
        }
    }

    pub(super) fn writer(&self) -> &TestTerminal {
        self.frame_renderer.writer()
    }

    pub(super) fn test_writer_mut(&mut self) -> &mut TestTerminal {
        self.frame_renderer.test_writer_mut()
    }

    pub(super) fn render(&mut self) -> std::io::Result<()> {
        let context = self.frame_renderer.context();
        let scrollback = self.app.drain_scrollback(&context);
        if !scrollback.is_empty() {
            self.frame_renderer.push_to_scrollback(&scrollback)?;
        }
        self.frame_renderer.render_frame(|ctx| self.app.render(ctx))
    }

    pub(super) fn initial_render(&mut self) -> std::io::Result<()> {
        self.render()
    }

    pub(super) async fn on_key_event(
        &mut self,
        key_event: tui::KeyEvent,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        self.handle_terminal_event(Event::Key(key_event)).await
    }

    pub(super) fn on_session_update(
        &mut self,
        update: acp::SessionUpdate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.handle_acp_event(AcpEvent::SessionUpdate(Box::new(update)))?;
        Ok(())
    }

    pub(super) fn on_prompt_done(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.handle_acp_event(AcpEvent::PromptDone(acp::StopReason::EndTurn))?;
        Ok(())
    }

    pub(super) async fn on_tick(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.handle_terminal_event(Event::Tick).await?;
        Ok(())
    }

    pub(super) async fn on_paste(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.handle_terminal_event(Event::Paste(text.to_string()))
            .await?;
        Ok(())
    }

    pub(super) async fn on_resize_event(
        &mut self,
        cols: u16,
        rows: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.frame_renderer.on_resize((cols, rows));
        self.handle_terminal_event(Event::Resize((cols, rows).into()))
            .await?;
        Ok(())
    }

    pub(super) fn on_ext_notification(
        &mut self,
        notification: acp::ExtNotification,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.handle_acp_event(AcpEvent::ExtNotification(notification))?;
        Ok(())
    }

    pub(super) fn on_connection_closed(
        &mut self,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        self.handle_acp_event(AcpEvent::ConnectionClosed)
    }

    async fn handle_terminal_event(
        &mut self,
        event: Event,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        let commands = self.app.on_event(&event).await.unwrap_or_default();
        self.drain_and_render(commands)
    }

    fn handle_acp_event(
        &mut self,
        event: AcpEvent,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        self.app.on_acp_event(event);
        self.drain_and_render(vec![])
    }

    fn drain_and_render(
        &mut self,
        commands: Vec<RendererCommand>,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        self.frame_renderer.apply_commands(commands)?;

        if self.app.exit_requested() {
            return Ok(LoopAction::Exit);
        }

        self.render()?;
        Ok(LoopAction::Continue)
    }
}

/// Test events that can be fed to the renderer.
pub(super) enum TestEvent {
    Update(Box<acp::SessionUpdate>),
    PromptDone,
}

/// Build the expected bordered prompt lines for a given terminal width.
/// Returns `[top_border, input_line, bottom_border, status_line]`.
pub(super) fn expected_prompt(width: u16, input: &str, agent_name: &str) -> Vec<String> {
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
pub(super) fn expected_with_prompt(
    scrollback: &[&str],
    width: u16,
    input: &str,
    agent_name: &str,
) -> Vec<String> {
    let mut lines: Vec<String> = scrollback.iter().map(ToString::to_string).collect();
    lines.extend(expected_prompt(width, input, agent_name));
    lines
}

pub(super) fn has_file_picker(terminal: &TestTerminal) -> bool {
    let lines = terminal.get_lines();
    lines
        .iter()
        .any(|l| l.contains("▶ ") || l.contains("(no matches found)"))
}

pub(super) fn has_command_picker(terminal: &TestTerminal) -> bool {
    let lines = terminal.get_lines();
    lines
        .iter()
        .any(|l| l.contains("Open configuration settings"))
}

pub(super) fn has_config_menu(terminal: &TestTerminal) -> bool {
    let lines = terminal.get_lines();
    lines.iter().any(|l| l.contains("Configuration"))
}

pub(super) fn has_config_picker(terminal: &TestTerminal) -> bool {
    let lines = terminal.get_lines();
    lines.iter().any(|l| l.contains("search:"))
}

pub(super) fn config_menu_selected_label(terminal: &TestTerminal) -> Option<String> {
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

pub(super) fn command_picker_visible_names(terminal: &TestTerminal) -> Vec<String> {
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

pub(super) fn render_with_size(events: Vec<TestEvent>, size: (u16, u16)) -> Renderer {
    let terminal = TestTerminal::new(size.0, size.1);
    let mut renderer = Renderer::new(terminal, TEST_AGENT.to_string(), &[], size);

    for event in events {
        match event {
            TestEvent::Update(update) => renderer.on_session_update(*update).unwrap(),
            TestEvent::PromptDone => renderer.on_prompt_done().unwrap(),
        }
    }

    renderer
}

pub(super) fn render(events: Vec<TestEvent>) -> Renderer {
    render_with_size(events, (TEST_WIDTH, 40))
}

pub(super) fn text_chunk(text: &str) -> TestEvent {
    TestEvent::Update(Box::new(acp::SessionUpdate::AgentMessageChunk(
        acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(text))),
    )))
}

pub(super) fn thought_chunk(text: &str) -> TestEvent {
    TestEvent::Update(Box::new(acp::SessionUpdate::AgentThoughtChunk(
        acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(text))),
    )))
}

pub(super) fn prompt_done() -> TestEvent {
    TestEvent::PromptDone
}

pub(super) fn tool_call(name: &str, args: &str) -> TestEvent {
    tool_call_with_id(name, &format!("call_{name}"), args)
}

pub(super) fn tool_call_with_id(name: &str, id: &str, args: &str) -> TestEvent {
    let mut tc = acp::ToolCall::new(id.to_string(), name);
    if !args.is_empty() {
        let value: serde_json::Value = serde_json::from_str(args)
            .unwrap_or_else(|_| serde_json::Value::String(args.to_string()));
        tc = tc.raw_input(value);
    }
    TestEvent::Update(Box::new(acp::SessionUpdate::ToolCall(tc)))
}

pub(super) fn tool_complete(id: &str) -> TestEvent {
    TestEvent::Update(Box::new(acp::SessionUpdate::ToolCallUpdate(
        acp::ToolCallUpdate::new(
            id.to_string(),
            acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::Completed),
        ),
    )))
}

pub(super) fn tool_complete_with_display_meta(
    id: &str,
    display_meta: &serde_json::Value,
) -> TestEvent {
    let title = display_meta["title"].as_str().unwrap_or("");
    let value = display_meta["value"].as_str().unwrap_or("");

    let mut meta_map = serde_json::Map::new();
    if !value.is_empty() {
        meta_map.insert("display_value".into(), value.into());
    }

    let mut update = acp::ToolCallUpdate::new(
        id.to_string(),
        acp::ToolCallUpdateFields::new()
            .title(title)
            .status(acp::ToolCallStatus::Completed),
    );
    if !meta_map.is_empty() {
        update = update.meta(meta_map);
    }
    TestEvent::Update(Box::new(acp::SessionUpdate::ToolCallUpdate(update)))
}

pub(super) fn tool_update_with_args(id: &str, args: &str) -> TestEvent {
    let value: serde_json::Value = serde_json::from_str(args).unwrap();
    TestEvent::Update(Box::new(acp::SessionUpdate::ToolCallUpdate(
        acp::ToolCallUpdate::new(
            id.to_string(),
            acp::ToolCallUpdateFields::new().raw_input(value),
        ),
    )))
}

pub(super) async fn type_string(renderer: &mut Renderer, text: &str) {
    for ch in text.chars() {
        let key_event = KeyEvent {
            code: KeyCode::Char(ch),
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        renderer.on_key_event(key_event).await.unwrap();
    }
}

pub(super) async fn press_enter(renderer: &mut Renderer) {
    let enter_event = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };
    renderer.on_key_event(enter_event).await.unwrap();
}

pub(super) async fn press_backspace(renderer: &mut Renderer) {
    let backspace_event = KeyEvent {
        code: KeyCode::Backspace,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };
    renderer.on_key_event(backspace_event).await.unwrap();
}

pub(super) async fn send_key(renderer: &mut Renderer, code: KeyCode, modifiers: KeyModifiers) {
    renderer
        .on_key_event(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        })
        .await
        .unwrap();
}

pub(super) fn make_config_options() -> Vec<acp::SessionConfigOption> {
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
