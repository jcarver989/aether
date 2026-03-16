mod cli;
mod components;
mod error;
mod git_diff;
mod keybindings;
mod runtime_state;
mod settings;
#[cfg(test)]
mod test_helpers;

use crate::cli::Cli;
use crate::components::app::App;
use crate::error::AppError;
use crate::runtime_state::RuntimeState;
use tui::{
    Component, CrosstermEvent, Event, MouseCapture, Renderer, TerminalSession,
    spawn_terminal_event_task, terminal_size,
};
use clap::Parser;
use std::fs::create_dir_all;
use std::future::pending;
use std::io;
use std::process::ExitCode;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio::{select, time};
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

    let app = App::new(
        session_id,
        agent_name,
        &config_options,
        auth_methods,
        working_dir,
        prompt_handle,
    );

    match run_app(app, theme, event_rx).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fatal error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn render(renderer: &mut Renderer<impl io::Write>, app: &mut App) -> Result<(), AppError> {
    let context = renderer.context();
    let scrollback = app.drain_scrollback(&context);
    if !scrollback.is_empty() {
        renderer.push_to_scrollback(&scrollback)?;
    }
    renderer.render_frame(|ctx| app.render(ctx))?;
    Ok(())
}

async fn run_app(
    mut app: App,
    theme: tui::Theme,
    mut event_rx: mpsc::UnboundedReceiver<acp_utils::client::AcpEvent>,
) -> Result<(), AppError> {
    let size = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(io::stdout(), theme, size);
    let _session = TerminalSession::new(true, MouseCapture::Disabled)?;
    let mut terminal_rx = spawn_terminal_event_task();
    let mut tick_interval = {
        let mut tick = interval(Duration::from_millis(100));
        tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        tick
    };

    render(&mut renderer, &mut app)?;
    loop {
        let tick_fut = async {
            if !app.wants_tick() {
                pending::<()>().await;
            }
            tick_interval.tick().await;
        };

        select! {
            terminal_event = terminal_rx.recv() => {
                let Some(event) = terminal_event else {
                    return Ok(());
                };
                if let CrosstermEvent::Resize(cols, rows) = &event {
                    renderer.on_resize((*cols, *rows));
                }
                if let Ok(tui_event) = Event::try_from(event) {
                    let commands = app.on_event(&tui_event).await.unwrap_or_default();
                    renderer.apply_commands(commands)?;
                    if app.exit_requested() { return Ok(()); }
                    render(&mut renderer, &mut app)?;
                }
            }

            app_event = event_rx.recv() => {
                match app_event {
                    Some(event) => {
                        app.on_acp_event(event);
                        if app.exit_requested() { return Ok(()); }
                        render(&mut renderer, &mut app)?;
                    }
                    None => return Ok(()),
                }
            }

            () = tick_fut => {
                app.on_event(&Event::Tick).await;
                if app.exit_requested() { return Ok(()); }
                render(&mut renderer, &mut app)?;
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
