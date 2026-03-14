mod cli;
mod components;
mod error;
mod git_diff;
mod keybindings;
mod runtime_state;
mod settings;
#[cfg(test)]
mod test_helpers;
mod tui;

use crate::cli::Cli;
use crate::components::app::{GitDiffMode, UiState, UiStateController, UiView, WispEvent};
use crate::error::AppError;
use crate::runtime_state::RuntimeState;
use crate::tui::Event;
use crate::tui::advanced::{
    CrosstermEvent, MouseCapture, Renderer, TerminalSession, spawn_terminal_event_task,
    terminal_size,
};
use clap::Parser;
use std::fs::create_dir_all;
use std::io;
use std::process::ExitCode;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
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
        working_dir,
    } = state;

    let ui_state = UiState::new(agent_name, &config_options, auth_methods);
    let controller = UiStateController::new(session_id, prompt_handle);
    let git_diff_mode = GitDiffMode::new(working_dir);

    match run_app(ui_state, controller, git_diff_mode, theme, event_rx).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fatal error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run_app(
    mut state: UiState,
    mut controller: UiStateController,
    git_diff_mode: GitDiffMode,
    theme: tui::Theme,
    mut event_rx: mpsc::UnboundedReceiver<acp_utils::client::AcpEvent>,
) -> Result<(), AppError> {
    let _session = TerminalSession::enter(true, MouseCapture::Disabled)?;
    let mut view = UiView::new(Renderer::new(io::stdout(), theme), git_diff_mode);
    let size = terminal_size().unwrap_or((80, 24));
    view.on_resize(size);

    let mut terminal_rx = spawn_terminal_event_task();
    view.render(&mut state)?;

    let tick_rate = Duration::from_millis(100);
    let mut tick_interval = time::interval(tick_rate);
    tick_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        let tick_fut = async {
            if !state.wants_tick() {
                std::future::pending::<()>().await;
            }
            tick_interval.tick().await;
        };

        let external_fut = event_rx.recv();

        tokio::select! {
            terminal_event = terminal_rx.recv() => {
                let Some(event) = terminal_event else {
                    return Ok(());
                };
                if let CrosstermEvent::Resize(cols, rows) = &event {
                    view.on_resize((*cols, *rows));
                }
                if let Ok(tui_event) = Event::try_from(event) {
                    controller.handle_event(&mut state, &mut view, WispEvent::Terminal(tui_event)).await?;
                    if state.exit_requested { return Ok(()); }
                    view.render(&mut state)?;
                }
            }

            app_event = external_fut => {
                match app_event {
                    Some(event) => {
                        controller.handle_event(&mut state, &mut view, WispEvent::Acp(event)).await?;
                        if state.exit_requested { return Ok(()); }
                        view.render(&mut state)?;
                    }
                    None => return Ok(()),
                }
            }

            () = tick_fut => {
                controller.handle_event(&mut state, &mut view, WispEvent::Terminal(Event::Tick)).await?;
                if state.exit_requested { return Ok(()); }
                view.render(&mut state)?;
            }
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
