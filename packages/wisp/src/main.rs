mod app_state;
mod cli;
mod components;
mod error;
mod renderer;
mod tui;

use acp_utils::client::AcpEvent;
use agent_client_protocol as acp;
use clap::Parser;
use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind, poll, read,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use renderer::LoopAction;
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;
use tokio::{select, time};

use crate::app_state::AppState;
use crate::cli::Cli;
use crate::renderer::Renderer;

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

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let log_dir = cli
        .log_dir
        .clone()
        .unwrap_or_else(|| "/tmp/wisp-logs".to_string());
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = tracing_appender::rolling::daily(&log_dir, "wisp.log");
    tracing_subscriber::fmt()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let state = match AppState::from_cli(&cli).await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Failed to initialize: {e}");
            return ExitCode::FAILURE;
        }
    };

    let result = if cli.prompt.is_empty() {
        run_terminal_ui(state).await.map(|_| ExitCode::SUCCESS)
    } else {
        let prompt = cli.prompt.join(" ");
        run_non_interactive(state, &prompt).await
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Fatal error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run_terminal_ui(mut state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnableBracketedPaste)?;
    let mut renderer = Renderer::new(
        io::stdout(),
        state.agent_name.clone(),
        &state.config_options,
    );

    renderer.update_render_context();
    renderer.initial_render()?;

    loop {
        select! {
            Some(event) = state.event_rx.recv() => {
                match event {
                    AcpEvent::SessionUpdate(update) => {
                        if let Err(e) = renderer.on_session_update(*update) {
                            eprintln!("Error handling session update: {e}");
                        }
                    }
                    AcpEvent::PromptDone(_stop_reason) => {
                        if let Err(e) = renderer.on_prompt_done() {
                            eprintln!("Error handling prompt done: {e}");
                        }
                    }
                    AcpEvent::PromptError(e) => {
                        if let Err(render_err) = renderer.on_prompt_error() {
                            eprintln!("Error handling prompt error render: {render_err}");
                        }
                        eprintln!("Prompt error: {e}");
                    }
                    AcpEvent::ConnectionClosed => {
                        break;
                    }
                }
            }

            _ = time::sleep(Duration::from_millis(50)) => {
                if let Err(e) = renderer.on_tick() {
                    eprintln!("Error on tick: {e}");
                }
                if let Ok(true) = poll(Duration::from_millis(0)) {
                    match read() {
                        Ok(Event::Key(key_event)) => {
                            if key_event.kind == KeyEventKind::Press {
                                match renderer.on_key_event(
                                    key_event,
                                    &state.prompt_handle,
                                    &state.session_id,
                                ) {
                                    Ok(LoopAction::Exit) => {
                                        break;
                                    }
                                    Ok(LoopAction::Continue) => {}
                                    Err(e) => {
                                        eprintln!("Error handling key event: {e}");
                                    }
                                }
                            }
                        }
                        Ok(Event::Paste(text)) => {
                            if let Err(e) = renderer.on_paste(&text) {
                                eprintln!("Error handling paste: {e}");
                            }
                        }
                        Ok(Event::Resize(cols, rows)) => {
                            if let Err(e) = renderer.on_resize(cols, rows) {
                                eprintln!("Error handling resize: {e}");
                            }
                        }
                        Err(e) => {
                            eprintln!("Event read error: {e}");
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    crossterm::execute!(io::stdout(), DisableBracketedPaste)?;
    disable_raw_mode()?;
    println!("\nGoodbye!");
    Ok(())
}

async fn run_non_interactive(
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
                    if let acp::ContentBlock::Text(text_content) = chunk.content {
                        if let Some(output) = thought_state.on_thought_chunk(&text_content.text) {
                            print!("{output}");
                            io::stdout().flush()?;
                        }
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
