use crate::app_state::AppState;
use acp_utils::client::AcpEvent;
use agent_client_protocol as acp;
use std::io::{self, Write};
use std::process::ExitCode;

#[derive(Debug, Clone)]
struct NonInteractiveThoughtState {
    in_block: bool,
    block_ends_with_newline: bool,
}

impl Default for NonInteractiveThoughtState {
    fn default() -> Self {
        Self {
            in_block: false,
            block_ends_with_newline: true,
        }
    }
}

impl NonInteractiveThoughtState {
    fn on_thought_chunk(&mut self, chunk: &str) -> Option<String> {
        if chunk.is_empty() {
            return None;
        }

        let mut output = String::new();
        if !self.in_block {
            output.push_str("Thought: ");
            self.in_block = true;
        }

        output.push_str(chunk);
        self.block_ends_with_newline = chunk.ends_with('\n');
        Some(output)
    }

    fn on_non_thought_update(&mut self) -> Option<&'static str> {
        if !self.in_block {
            return None;
        }

        self.in_block = false;
        if self.block_ends_with_newline {
            self.block_ends_with_newline = true;
            None
        } else {
            self.block_ends_with_newline = true;
            Some("\n")
        }
    }
}

pub(crate) async fn run_non_interactive(
    mut state: AppState,
    prompt: &str,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut thought_state = NonInteractiveThoughtState::default();

    state
        .prompt_handle
        .prompt(&state.session_id, prompt, None)?;

    while let Some(event) = state.event_rx.recv().await {
        match event {
            AcpEvent::SessionUpdate(update) => match *update {
                acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                    if let acp::ContentBlock::Text(text_content) = chunk.content
                        && let Some(output) = thought_state.on_thought_chunk(&text_content.text)
                    {
                        print!("{output}");
                        io::stdout().flush()?;
                    }
                }

                non_thought_update => {
                    if let Some(output) = thought_state.on_non_thought_update() {
                        print!("{output}");
                        io::stdout().flush()?;
                    }

                    match non_thought_update {
                        acp::SessionUpdate::AgentMessageChunk(chunk) => {
                            if let acp::ContentBlock::Text(text_content) = chunk.content {
                                print!("{}", text_content.text);
                                io::stdout().flush()?;
                            }
                        }
                        acp::SessionUpdate::ToolCall(tool_call) => {
                            println!("[Tool: {}] Starting...", tool_call.title);
                        }
                        acp::SessionUpdate::ToolCallUpdate(update) => {
                            if let Some(status) = &update.fields.status {
                                match status {
                                    acp::ToolCallStatus::Completed => {
                                        println!("[Tool: {}] ✓ Completed", update.tool_call_id);
                                    }
                                    acp::ToolCallStatus::Failed => {
                                        eprintln!("[Tool: {}] ✗ Failed", update.tool_call_id);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            },
            AcpEvent::ExtNotification(_) => {}
            AcpEvent::PromptDone(_) => {
                if let Some(output) = thought_state.on_non_thought_update() {
                    print!("{output}");
                    io::stdout().flush()?;
                }
                println!();
                break;
            }
            AcpEvent::PromptError(e) => {
                if let Some(output) = thought_state.on_non_thought_update() {
                    print!("{output}");
                    io::stdout().flush()?;
                }
                eprintln!("Error: {e}");
                return Ok(ExitCode::FAILURE);
            }
            AcpEvent::ConnectionClosed => {
                if let Some(output) = thought_state.on_non_thought_update() {
                    print!("{output}");
                    io::stdout().flush()?;
                }
                break;
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::NonInteractiveThoughtState;

    #[test]
    fn non_interactive_prefixes_once_per_contiguous_block() {
        let mut state = NonInteractiveThoughtState::default();
        assert_eq!(
            state.on_thought_chunk("plan"),
            Some("Thought: plan".to_string())
        );
        assert_eq!(state.on_thought_chunk(" more"), Some(" more".to_string()));
        assert_eq!(state.on_non_thought_update(), Some("\n"));
        assert_eq!(
            state.on_thought_chunk("next"),
            Some("Thought: next".to_string())
        );
    }

    #[test]
    fn non_interactive_does_not_emit_extra_newline_when_chunk_ends_with_newline() {
        let mut state = NonInteractiveThoughtState::default();
        assert_eq!(
            state.on_thought_chunk("plan\n"),
            Some("Thought: plan\n".to_string())
        );
        assert_eq!(state.on_non_thought_update(), None);
    }
}
