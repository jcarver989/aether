mod cli;
mod components;
mod error;
mod keybindings;
mod runtime_state;
mod settings;
#[cfg(test)]
mod test_helpers;
mod tui;

use crate::cli::Cli;
use crate::components::app::App;
use crate::runtime_state::RuntimeState;
use crate::tui::{Renderer, TerminalSession, run_app, spawn_terminal_event_task, terminal_size};
use clap::Parser;
use std::fs::create_dir_all;
use std::process::ExitCode;
use tracing_appender::rolling::daily;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    setup_logging(&cli);

    let state = match RuntimeState::from_cli(&cli).await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Failed to initialize: {e}");
            return ExitCode::FAILURE;
        }
    };

    let RuntimeState {
        session_id,
        agent_name,
        config_options,
        auth_methods,
        theme,
        event_rx,
        prompt_handle,
    } = state;

    let mut app = App::new(
        agent_name,
        &config_options,
        auth_methods,
        prompt_handle,
        session_id,
    );

    let _session = match TerminalSession::enter(true) {
        Ok(session) => session,
        Err(e) => {
            eprintln!("Failed to enter terminal: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut renderer = Renderer::new(std::io::stdout(), theme);
    let size = terminal_size().unwrap_or((80, 24));
    renderer.on_resize(size);
    let terminal_rx = spawn_terminal_event_task();

    match run_app(
        &mut app,
        &mut renderer,
        terminal_rx,
        Some(event_rx),
        Some(std::time::Duration::from_millis(100)),
    )
    .await
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fatal error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn setup_logging(cli: &Cli) {
    let log_dir = cli
        .log_dir
        .clone()
        .unwrap_or_else(|| "/tmp/wisp-logs".to_string());

    create_dir_all(&log_dir).ok();
    tracing_subscriber::fmt()
        .with_writer(daily(&log_dir, "wisp.log"))
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}
