use acp_utils::client::AcpPromptHandle;
use crate::components::grid_loader::GridLoader;
use crate::components::input_prompt::InputPrompt;
use crate::components::status_line::StatusLine;
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::tui::{Container, FrameRenderer, Line, Screen};
use agent_client_protocol::{self as acp, SessionConfigOption};
use crossterm::event::{self, KeyCode, KeyEvent};
use std::io::Write;

pub enum LoopAction {
    Continue,
    Exit,
}

pub struct Renderer<T: Write> {
    tui: FrameRenderer<T>,
    tool_call_statuses: ToolCallStatuses,
    grid_loader: GridLoader,
    current_message_buffer: String,
    input_buffer: String,
    agent_name: String,
    model_display: Option<String>,
    waiting_for_response: bool,
    animation_tick: u16,
}

impl<T: Write> Renderer<T> {
    pub fn new(writer: T, agent_name: String, config_options: &[SessionConfigOption]) -> Self {
        Self {
            tui: FrameRenderer::new(writer),
            tool_call_statuses: ToolCallStatuses::new(),
            grid_loader: GridLoader::new(),
            current_message_buffer: String::new(),
            input_buffer: String::new(),
            agent_name,
            model_display: extract_model_display(config_options),
            waiting_for_response: false,
            animation_tick: 0,
        }
    }

    /// Get a reference to the writer (useful for testing)
    #[allow(dead_code)]
    pub fn writer(&self) -> &T {
        self.tui.writer()
    }

    /// Get a mutable reference to the writer (useful for testing)
    #[allow(dead_code)]
    pub fn writer_mut(&mut self) -> &mut T {
        self.tui.writer_mut()
    }

    /// Position the cursor inside the bordered input box.
    /// Moves up 2 lines (past bottom border + status line) and right to the input position.
    fn position_cursor_in_input(&mut self) -> std::io::Result<()> {
        // "│ > " is 4 visual columns, then the input text
        let col = 4 + self.input_buffer.len() as u16;
        self.tui.reposition_cursor(2, col)
    }

    /// Render the current frame to the terminal.
    fn render_frame(&mut self) -> std::io::Result<()> {
        let mut root = Container::new();
        root.add(&self.tool_call_statuses);
        root.add(&self.grid_loader);
        let input_prompt = InputPrompt {
            input: &self.input_buffer,
        };
        let status_line = StatusLine {
            agent_name: &self.agent_name,
            model_display: self.model_display.as_deref(),
        };
        root.add(&input_prompt);
        root.add(&status_line);
        self.tui.render_frame(&root)?;
        self.position_cursor_in_input()?;
        Ok(())
    }

    pub fn on_key_event(
        &mut self,
        key_event: KeyEvent,
        prompt_handle: &AcpPromptHandle,
        session_id: &acp::SessionId,
    ) -> Result<LoopAction, std::io::Error> {
        match key_event.code {
            KeyCode::Char('c') if key_event.modifiers.contains(event::KeyModifiers::CONTROL) => {
                return Ok(LoopAction::Exit);
            }

            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                self.render_frame()?;
            }

            KeyCode::Backspace => {
                if !self.input_buffer.is_empty() {
                    self.input_buffer.pop();
                    self.render_frame()?;
                }
            }

            KeyCode::Enter => {
                if self.input_buffer.trim().is_empty() {
                    // Just re-render the prompt
                    self.render_frame()?;
                } else {
                    let user_input = self.input_buffer.trim().to_string();
                    self.input_buffer.clear();

                    // Push the user's message line to scrollback
                    self.tui
                        .push_to_scrollback(&[Line::new(user_input.clone())])?;

                    prompt_handle.prompt(session_id, &user_input);

                    self.waiting_for_response = true;
                    self.animation_tick = 0;
                    self.grid_loader.visible = true;
                    self.grid_loader.tick = 0;

                    // Render fresh prompt
                    self.render_frame()?;
                }
            }
            _ => {}
        }
        Ok(LoopAction::Continue)
    }

    /// Handle a streaming session update from ACP.
    pub fn on_session_update(&mut self, update: acp::SessionUpdate) -> std::io::Result<()> {
        let was_loading = self.grid_loader.visible;
        self.waiting_for_response = false;
        self.grid_loader.visible = false;

        match update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                if let acp::ContentBlock::Text(text_content) = chunk.content {
                    self.current_message_buffer.push_str(&text_content.text);
                }
            }

            acp::SessionUpdate::ToolCall(tool_call) => {
                self.tool_call_statuses.on_tool_call(&tool_call);
                self.render_frame()?;
            }

            acp::SessionUpdate::ToolCallUpdate(update) => {
                self.tool_call_statuses.on_tool_call_update(&update);
                self.render_frame()?;
            }

            acp::SessionUpdate::ConfigOptionUpdate(update) => {
                self.model_display = extract_model_display(&update.config_options);
                self.render_frame()?;
            }

            _ => {}
        }

        // If the loader was just dismissed, ensure the frame is re-rendered.
        // Match arms that already called render_frame() make this a no-op
        // via Screen's frame deduplication.
        if was_loading {
            self.render_frame()?;
        }

        Ok(())
    }

    /// Called when the agent's prompt response is complete.
    /// Flushes accumulated text and completed tool calls to scrollback.
    pub fn on_prompt_done(&mut self) -> std::io::Result<()> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;

        let text = std::mem::take(&mut self.current_message_buffer);

        let mut scrollback_lines: Vec<Line> =
            self.tool_call_statuses.drain_completed(self.tui.context());

        if !text.is_empty() {
            for text_line in text.lines() {
                scrollback_lines.push(Line::new(text_line.to_string()));
            }
        }

        if !scrollback_lines.is_empty() {
            self.tui.push_to_scrollback(&scrollback_lines)?;
        }

        self.render_frame()?;
        Ok(())
    }

    /// Advance the loader animation by one tick. No-op when not waiting.
    pub fn on_tick(&mut self) -> std::io::Result<()> {
        if self.waiting_for_response {
            self.animation_tick = self.animation_tick.wrapping_add(1);
            self.grid_loader.tick = self.animation_tick;
            self.render_frame()?;
        }
        Ok(())
    }

    /// Called when the agent prompt errors out.
    pub fn on_prompt_error(&mut self) -> std::io::Result<()> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        self.render_frame()
    }

    pub fn update_render_context(&mut self) {
        self.tui.update_context_from_terminal();
    }

    /// Handle terminal resize: update context and re-render.
    pub fn on_resize(&mut self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.tui.update_context((cols, rows));
        self.render_frame()
    }

    /// Update render context with provided size (useful for testing)
    #[allow(dead_code)]
    pub fn update_render_context_with(&mut self, size: (u16, u16)) {
        self.tui.update_context(size);
    }

    /// Render the initial frame (just the prompt).
    pub fn initial_render(&mut self) -> std::io::Result<()> {
        self.render_frame()
    }

    /// Get a reference to the screen (useful for testing)
    #[allow(dead_code)]
    pub fn screen(&self) -> &Screen {
        self.tui.screen()
    }
}

/// Extract a human-readable model display string from config options.
///
/// Finds the first option with `category == Model`, reads its `Select` kind,
/// and looks up the `current_value` in the options list to return its `name`.
fn extract_model_display(config_options: &[acp::SessionConfigOption]) -> Option<String> {
    let option = config_options.iter().find(|o| {
        o.category
            .as_ref()
            .is_some_and(|c| *c == acp::SessionConfigOptionCategory::Model)
    })?;

    let acp::SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    let acp::SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
        return None;
    };

    options
        .iter()
        .find(|o| o.value == select.current_value)
        .map(|o| o.name.clone())
}
