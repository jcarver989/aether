mod cli;
mod components;
mod output_formatters;
mod render_context;
mod renderer;

use std::io;

use clap::Parser;
use crossterm::cursor::{MoveTo, position};
use crossterm::event::{Event, KeyEventKind, poll, read};
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode};
use render_context::RenderContext;
use renderer::LoopAction;
use std::time::Duration;
use tracing_subscriber;
mod app_state;

use crate::app_state::AppState;
use crate::cli::Cli;
use crate::components::commands::ExecuteCommands;
use crate::components::input_prompt::InputPrompt;
use crate::render_context::Component;
use crate::renderer::Renderer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing - set RUST_LOG env var to control log level
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let state = AppState::from_cli(&cli).await?;

    run_terminal_ui(state).await?;

    Ok(())
}

async fn run_terminal_ui(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

    let input_prompt = InputPrompt {};
    let context = RenderContext::new(position()?);
    stdout.flush_commands(&input_prompt.render((), &context))?;

    let user_msg_tx = state.agent_tx;
    let mut agent_msg_rx = state.agent_rx;
    let mut renderer = Renderer::new(stdout);

    loop {
        renderer.update_render_context();
        tokio::select! {
            Some(message) = agent_msg_rx.recv() => {
                if let Err(e) = renderer.on_agent_message(message).await {
                    eprintln!("Error handling agent message: {}", e);
                }
            }

            _ = tokio::time::sleep(Duration::from_millis(50)) => {
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
                                        eprintln!("Error handling key event: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Event read error: {}", e);
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
