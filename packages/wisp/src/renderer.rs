use std::io::{Stdout, Write};

use aether::agent::{AgentMessage, UserMessage};
use crossterm::{
    cursor::position,
    event::{self, KeyCode, KeyEvent},
    terminal::size,
};
use tokio::sync::mpsc;

use crate::{
    components::{
        agent_text_message::AgentTextMessage,
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

pub struct Renderer {
    stdout: Stdout,
    tool_call_statuses: ToolCallStatuses,
    current_assistant_message_id: Option<String>,
    input_buffer: String,
    context: RenderContext,
}

impl Renderer {
    pub fn new(stdout: Stdout) -> Self {
        Self {
            stdout,
            tool_call_statuses: ToolCallStatuses::new(),
            current_assistant_message_id: None,
            input_buffer: String::new(),
            context: RenderContext::new((0, 0), (0, 0)),
        }
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
                self.stdout.flush()?;
            }

            KeyCode::Backspace => {
                if !self.input_buffer.is_empty() {
                    self.input_buffer.pop();
                    self.stdout.flush_commands(&[
                        TerminalCommand::MoveLeft,
                        TerminalCommand::Print(" ".to_string()),
                        TerminalCommand::MoveLeft,
                    ])?;
                    self.stdout.flush()?;
                }
            }

            KeyCode::Enter => {
                if self.input_buffer.trim().is_empty() {
                    let input_prompt = InputPrompt {};
                    let commands = input_prompt.render((), &self.context);
                    self.stdout.flush_commands(&commands)?;
                } else {
                    let user_input = self.input_buffer.trim().to_string();
                    self.stdout.flush_commands(&[
                        TerminalCommand::Print("\r\n\r\n".to_string()),
                        TerminalCommand::MoveToColumn(1),
                    ])?;

                    if let Err(e) = tx
                        .send(UserMessage::Text {
                            content: user_input,
                        })
                        .await
                    {
                        eprintln!("Failed to send message: {}", e);
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
        match message {
            AgentMessage::Text {
                message_id,
                chunk,
                is_complete,
                ..
            } => {
                let mut commands = vec![];
                if self.current_assistant_message_id.as_ref() != Some(&message_id) && !is_complete {
                    commands.extend(AgentTextMessage::render());
                    self.current_assistant_message_id = Some(message_id.clone());
                }

                if !is_complete {
                    for (i, line) in chunk.split('\n').enumerate() {
                        if i > 0 {
                            commands.push(TerminalCommand::Print("\r\n".to_string()));
                            commands.push(TerminalCommand::MoveToColumn(1));
                        }
                        if !line.is_empty() {
                            commands.push(TerminalCommand::Print(line.to_string()));
                        }
                    }
                }

                if is_complete {
                    commands.push(TerminalCommand::Print("\r\n".to_string()));
                    let input_prompt = InputPrompt {};
                    commands.extend(input_prompt.render((), &self.context));
                    self.current_assistant_message_id = None;
                }

                self.stdout.flush_commands(&commands)?;
            }

            AgentMessage::ToolCall { request, .. } => {
                let commands = self
                    .tool_call_statuses
                    .on_tool_request(&request, &self.context);
                self.stdout.flush_commands(&commands)?;
            }

            AgentMessage::ToolResult { result, .. } => {
                let commands = self
                    .tool_call_statuses
                    .on_tool_result(&result, &self.context);
                self.stdout.flush_commands(&commands)?;
            }

            AgentMessage::ToolError { error, .. } => {
                let commands = self.tool_call_statuses.on_tool_error(&error, &self.context);
                self.stdout.flush_commands(&commands)?;
            }

            _ => {}
        }

        Ok(())
    }

    pub fn update_render_context(&mut self) -> () {
        let position = match position() {
            Ok(p) => p,
            Err(e) => {
                println!("Failed to get position: {}", e);
                (0, 1)
            }
        };

        let size = match size() {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to get size: {}", e);
                (1, 1)
            }
        };

        self.context = RenderContext::new(position, size);
    }
}
