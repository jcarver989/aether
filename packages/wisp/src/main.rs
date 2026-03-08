mod cli;
mod components;
mod error;
mod keybindings;
mod runtime_state;
mod settings;
mod terminal_ui;
#[cfg(test)]
mod test_helpers;
mod tui;

use crate::cli::Cli;
use crate::runtime_state::RuntimeState;
use crate::terminal_ui::run_terminal_ui;
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

    match run_terminal_ui(state).await {
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
