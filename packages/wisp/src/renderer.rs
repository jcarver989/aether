use std::io::Write;

use aether::agent::{AgentMessage, UserMessage};
use crossterm::{
    cursor::position,
    event::{self, KeyCode, KeyEvent},
    terminal::size,
};
use tokio::sync::mpsc;

use crate::{
    components::{
        commands::{ExecuteCommands, TerminalCommand},
        input_prompt::InputPrompt,
        tool_call_statuses::ToolCallStatuses,
    },
    render_context::{Component, RenderContext},
};

pub enum LoopAction {
    Continue,
    Exit,
}

pub struct Renderer<W: Write> {
    writer: W,
    tool_call_statuses: ToolCallStatuses,
    current_assistant_message_id: Option<String>,
    current_message_buffer: String,
    input_buffer: String,
    context: RenderContext,
}

impl<W: Write> Renderer<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            tool_call_statuses: ToolCallStatuses::new(),
            current_assistant_message_id: None,
            current_message_buffer: String::new(),
            input_buffer: String::new(),
            context: RenderContext::new((0, 0), (0, 0)),
        }
    }

    /// Get a reference to the writer (useful for testing)
    pub fn writer(&self) -> &W {
        &self.writer
    }

    /// Get a mutable reference to the writer (useful for testing)
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
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
                print!("{c}");
                self.writer.flush()?;
            }

            KeyCode::Backspace => {
                if !self.input_buffer.is_empty() {
                    self.input_buffer.pop();
                    self.writer.flush_commands(&[
                        TerminalCommand::MoveLeft,
                        TerminalCommand::Print(" ".to_string()),
                        TerminalCommand::MoveLeft,
                    ])?;
                    self.writer.flush()?;
                }
            }

            KeyCode::Enter => {
                if self.input_buffer.trim().is_empty() {
                    let input_prompt = InputPrompt {};
                    let commands = input_prompt.render((), &self.context);
                    self.writer.flush_commands(&commands)?;
                } else {
                    let user_input = self.input_buffer.trim().to_string();

                    if let Err(e) = tx
                        .send(UserMessage::Text {
                            content: user_input,
                        })
                        .await
                    {
                        eprintln!("Failed to send message: {e}");
                    }

                    self.input_buffer.clear();
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
        // The last line should be the input prompt
        // which we need to clear, print the agent message and re-render the prompt at the bottom of the screen
        let mut commands = vec![];

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
                    // Message complete - clear prompt line, print buffered message
                    // Note: InputPrompt adds a newline, so we don't add one here
                    let terminal_text = self.current_message_buffer.replace('\n', "\r\n");
                    commands.push(TerminalCommand::ClearLine);
                    commands.push(TerminalCommand::Print(terminal_text));
                    self.current_message_buffer.clear();
                    self.current_assistant_message_id = None;
                }
            }

            AgentMessage::ToolCall { request, .. } => {
                commands.push(TerminalCommand::MoveToColumn(0));
                commands.push(TerminalCommand::ClearLine);
                commands.extend(
                    self.tool_call_statuses
                        .on_tool_request(&request, &self.context),
                );
            }

            AgentMessage::ToolResult { result, .. } => {
                commands.extend(
                    self.tool_call_statuses
                        .on_tool_result(&result, &self.context),
                );
            }

            AgentMessage::ToolError { error, .. } => {
                commands.extend(self.tool_call_statuses.on_tool_error(&error, &self.context));
            }

            _ => {
                return Ok(());
            }
        }

        // Only render prompt if we have commands to execute
        if commands.len() > 0 {
            let input_prompt = InputPrompt {};
            commands.extend(input_prompt.render((), &self.context));
            self.writer.flush_commands(&commands)?;
        }

        Ok(())
    }

    pub fn update_render_context(&mut self) {
        let position = match position() {
            Ok(p) => p,
            Err(e) => {
                println!("Failed to get position: {e}");
                (0, 1)
            }
        };

        let size = match size() {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to get size: {e}");
                (1, 1)
            }
        };

        self.context = RenderContext::new(position, size);
    }

    /// Update render context with provided position and size (useful for testing)
    pub fn update_render_context_with(&mut self, position: (u16, u16), size: (u16, u16)) {
        self.context = RenderContext::new(position, size);
    }
}
