mod cli;
mod colors;
mod components;
mod output_formatters;
mod render_context;
mod renderer;
use clap::Parser;
use crossterm::cursor::{MoveTo, position};
use crossterm::event::{Event, KeyEventKind, poll, read};
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, size};
use render_context::RenderContext;
use renderer::LoopAction;
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;
use tokio::{select, time};
mod app_state;
use crate::app_state::AppState;
use crate::cli::Cli;
use crate::components::commands::ExecuteCommands;
use crate::components::input_prompt::InputPrompt;
use crate::render_context::Component;
use crate::renderer::Renderer;

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

    // Branch based on whether prompt was provided
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

async fn run_terminal_ui(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

    let input_prompt = InputPrompt {};
    let context = RenderContext::new(position()?, size()?);
    stdout.flush_commands(&input_prompt.render((), &context))?;

    let user_msg_tx = state.agent_tx;
    let mut agent_msg_rx = state.agent_rx;
    let mut renderer = Renderer::new(stdout);

    loop {
        renderer.update_render_context();
        select! {
            Some(message) = agent_msg_rx.recv() => {
                if let Err(e) = renderer.on_agent_message(message).await {
                    eprintln!("Error handling agent message: {e}");
                }
            }

            _ = time::sleep(Duration::from_millis(50)) => {
                if let Ok(true) = poll(Duration::from_millis(0)) {
                    match read() {
                        Ok(Event::Key(key_event)) => {
                            if key_event.kind == KeyEventKind::Press {
                                match renderer.on_key_event(key_event, &user_msg_tx).await {
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
                        Err(e) => {
                            eprintln!("Event read error: {e}");
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    println!("\nGoodbye!");
    Ok(())
}

async fn run_non_interactive(
    state: AppState,
    prompt: &str,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    use aether::agent::{AgentMessage, UserMessage};

    let user_msg_tx = state.agent_tx;
    let mut agent_msg_rx = state.agent_rx;

    // Send the initial prompt
    user_msg_tx
        .send(UserMessage::Text {
            content: prompt.to_string(),
        })
        .await
        .map_err(|e| format!("Failed to send prompt: {e}"))?;

    // Process agent messages until done
    while let Some(message) = agent_msg_rx.recv().await {
        match message {
            AgentMessage::Text {
                chunk, is_complete, ..
            } => {
                if is_complete {
                    println!();
                } else {
                    print!("{chunk}");
                    io::stdout().flush()?;
                }
            }

            AgentMessage::ToolCall { request, .. } => {
                println!("[Tool: {}] Starting...", request.name);
            }

            AgentMessage::ToolProgress {
                request,
                progress,
                total,
                message,
            } => {
                let progress_str = total
                    .map(|t| format!("{:.0}/{:.0}", progress, t))
                    .unwrap_or_else(|| format!("{:.0}", progress));
                let msg = message.as_deref().unwrap_or("");
                println!("[Tool: {}] {} {}", request.name, msg, progress_str);
            }

            AgentMessage::ToolResult { result, .. } => {
                println!("[Tool: {}] ✓ Completed", result.name);
            }

            AgentMessage::ToolError { error, .. } => {
                eprintln!("[Tool: {}] ✗ Error: {}", error.name, error.error);
            }

            AgentMessage::Error { message } => {
                eprintln!("Error: {message}");
                return Ok(ExitCode::FAILURE);
            }

            AgentMessage::Cancelled { message } => {
                eprintln!("Cancelled: {message}");
                return Ok(ExitCode::FAILURE);
            }

            AgentMessage::ContextCompactionStarted { message_count } => {
                println!("[Context compaction: {} messages]", message_count);
            }

            AgentMessage::ContextCompactionResult {
                messages_removed, ..
            } => {
                println!("[Context compacted: {} messages removed]", messages_removed);
            }

            AgentMessage::Done => {
                break;
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}
