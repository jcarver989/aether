use std::io::Write;

use agent_events::{AgentMessage, UserMessage};
use crossterm::event::{self, KeyCode, KeyEvent};
use crossterm::terminal::size;
use tokio::sync::mpsc;

use crate::components::input_prompt::InputPrompt;
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::render_context::{Component, RenderContext};
use crate::screen::{Line, Screen};

pub enum LoopAction {
    Continue,
    Exit,
}

pub struct Renderer<W: Write> {
    writer: W,
    screen: Screen,
    tool_call_statuses: ToolCallStatuses,
    current_assistant_message_id: Option<String>,
    current_message_buffer: String,
    input_buffer: String,
    context: RenderContext,
}

impl<W: Write> Renderer<W> {
    pub fn new(writer: W, origin_row: u16) -> Self {
        Self {
            writer,
            screen: Screen::new(origin_row),
            tool_call_statuses: ToolCallStatuses::new(),
            current_assistant_message_id: None,
            current_message_buffer: String::new(),
            input_buffer: String::new(),
            context: RenderContext::new((0, 0)),
        }
    }

    /// Get a reference to the writer (useful for testing)
    #[allow(dead_code)]
    pub fn writer(&self) -> &W {
        &self.writer
    }

    /// Get a mutable reference to the writer (useful for testing)
    #[allow(dead_code)]
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Build the current frame from component state.
    fn build_frame(&self) -> Vec<Line> {
        let mut lines = Vec::new();

        // Active tool calls
        lines.extend(self.tool_call_statuses.render(&self.context));

        // Input prompt (always at the bottom of the managed region)
        lines.extend(InputPrompt.render(&self.context));

        lines
    }

    /// Render the current frame to the terminal.
    fn render_frame(&mut self) -> std::io::Result<()> {
        let frame = self.build_frame();
        self.screen.render(&frame, &mut self.writer)?;
        Ok(())
    }

    pub async fn on_key_event(
        &mut self,
        key_event: KeyEvent,
        tx: &mpsc::Sender<UserMessage>,
    ) -> Result<LoopAction, Box<dyn std::error::Error>> {
        match key_event.code {
            KeyCode::Char('c') if key_event.modifiers.contains(event::KeyModifiers::CONTROL) => {
                return Ok(LoopAction::Exit);
            }

            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                // Append char inline — no need to re-render the whole frame
                write!(self.writer, "{c}")?;
                self.writer.flush()?;
            }

            KeyCode::Backspace => {
                if !self.input_buffer.is_empty() {
                    self.input_buffer.pop();
                    // Erase one char inline
                    write!(self.writer, "\x1b[D \x1b[D")?;
                    self.writer.flush()?;
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
                    self.screen.push_to_scrollback(
                        &[Line::new(user_input.clone())],
                        &mut self.writer,
                    )?;

                    if let Err(e) = tx
                        .send(UserMessage::Text {
                            content: user_input,
                        })
                        .await
                    {
                        eprintln!("Failed to send message: {e}");
                    }

                    // Render fresh prompt
                    self.render_frame()?;
                }
            }
            _ => {}
        }
        Ok(LoopAction::Continue)
    }

    pub async fn on_agent_message(
        &mut self,
        message: AgentMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match message {
            AgentMessage::Text {
                message_id,
                chunk,
                is_complete,
                ..
            } => {
                if self.current_assistant_message_id.as_ref() != Some(&message_id) {
                    self.current_message_buffer.clear();
                    self.current_assistant_message_id = Some(message_id.clone());
                }

                self.current_message_buffer.push_str(&chunk);

                if is_complete {
                    let text = std::mem::take(&mut self.current_message_buffer);
                    self.current_assistant_message_id = None;

                    // Push completed tool calls + text to scrollback,
                    // keeping Running tool calls in the managed frame.
                    let mut scrollback_lines: Vec<Line> =
                        self.tool_call_statuses.drain_completed(&self.context);

                    // Add the assistant text
                    for text_line in text.lines() {
                        scrollback_lines.push(Line::new(text_line.to_string()));
                    }

                    self.screen
                        .push_to_scrollback(&scrollback_lines, &mut self.writer)?;

                    // Render fresh prompt
                    self.render_frame()?;
                }
            }

            AgentMessage::ToolCall { request, .. } => {
                self.tool_call_statuses.on_tool_request(&request);
                self.render_frame()?;
            }

            AgentMessage::ToolProgress { request, .. } => {
                self.tool_call_statuses.on_tool_request(&request);
                self.render_frame()?;
            }

            AgentMessage::ToolResult { result, .. } => {
                self.tool_call_statuses.on_tool_result(&result);
                self.render_frame()?;
            }

            AgentMessage::ToolError { error, .. } => {
                self.tool_call_statuses.on_tool_error(&error);
                self.render_frame()?;
            }

            _ => {}
        }

        Ok(())
    }

    pub fn update_render_context(&mut self) {
        let sz = match size() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to get size: {e}");
                (80, 24)
            }
        };
        self.context = RenderContext::new(sz);
    }

    /// Update render context with provided size (useful for testing)
    #[allow(dead_code)]
    pub fn update_render_context_with(&mut self, size: (u16, u16)) {
        self.context = RenderContext::new(size);
    }

    /// Render the initial frame (just the prompt).
    pub fn initial_render(&mut self) -> std::io::Result<()> {
        self.render_frame()
    }

    /// Get a reference to the screen (useful for testing)
    #[allow(dead_code)]
    pub fn screen(&self) -> &Screen {
        &self.screen
    }
}
