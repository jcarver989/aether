mod app_view;
mod cli;
mod colors;
mod components;
mod simple_input;
mod ui;

use aether::agent::{AgentMessage, UserMessage};
use clap::Parser;

use std::io::Write;
use tracing_subscriber;
mod app_state;

use crate::app_state::AppState;
use crate::app_view::AppView;
use crate::cli::Cli;
use crate::components::{Input, Logo, Screen};
use crate::simple_input::{InputResult, SimpleInput};
use iocraft::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing - set RUST_LOG env var to control log level
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let state = AppState::from_cli(&cli).await?;

    tokio::spawn(async move {
        element! {
            Screen()
        }
        .render_loop()
        .await
    });

    return run_interactive_mode(state).await;
}

async fn run_interactive_mode(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let init_display_name = ui::format_model_display_name(&state.model_specs);

    let mut app_view = AppView::new();
    let mut rx = state.agent.rx;
    let tx = state.agent.tx;

    loop {
        // Get input from user - SimpleInput no longer manages raw mode
        let mut input = SimpleInput::new();
        let user_input = match input.run_with_raw_mode_managed()? {
            InputResult::Submit(content) => content,
            InputResult::Exit => break,
            InputResult::Cancel => continue,
        };

        // Reset app view state for new conversation
        app_view.reset_for_new_conversation();

        // Send message to agent
        tx.send(UserMessage::text(&user_input)).await?;

        // Process agent responses until agent is completely done
        while let Some(event) = rx.recv().await {
            tracing::trace!("Received agent event: {:?}", std::mem::discriminant(&event));
            let is_done = matches!(event, AgentMessage::Done);
            app_view.update(event)?;
            if is_done {
                break;
            }
        }

        // Agent is done - clean up any remaining spinners
        app_view.stop_all_spinners()?;
        std::io::stdout().flush()?;
    }

    state.agent.task_handle.abort();
    ui::show_completion()?;
    Ok(())
}
