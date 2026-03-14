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
use crate::components::app::{App, AppAction, WispEvent};
use crate::runtime_state::RuntimeState;
use crate::tui::Event;
use crate::tui::advanced::{
    CrosstermEvent, MouseCapture, Renderer, TerminalSession, spawn_terminal_event_task,
    terminal_size,
};
use clap::Parser;
use std::collections::VecDeque;
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

    let app = App::new(
        agent_name,
        &config_options,
        auth_methods,
        prompt_handle,
        session_id,
        working_dir,
    );

    match run_app(app, theme, event_rx).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fatal error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run_app(
    mut app: App,
    theme: tui::Theme,
    mut event_rx: mpsc::UnboundedReceiver<acp_utils::client::AcpEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let _session = TerminalSession::enter(true, MouseCapture::Disabled)?;
    let mut renderer = Renderer::new(io::stdout(), theme);
    let size = terminal_size().unwrap_or((80, 24));
    renderer.on_resize(size);

    let mut terminal_rx = spawn_terminal_event_task();
    renderer.render_frame(|ctx| app.view(ctx))?;

    let tick_rate = Duration::from_millis(100);
    let mut tick_interval = time::interval(tick_rate);
    tick_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        let tick_fut = async {
            if !app.wants_tick() {
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
                    renderer.on_resize((*cols, *rows));
                }
                if let Ok(tui_event) = Event::try_from(event)
                    && handle_event(&mut app, &mut renderer, WispEvent::Terminal(tui_event)).await?
                {
                    return Ok(());
                }
            }

            app_event = external_fut => {
                match app_event {
                    Some(event) => {
                        if handle_event(&mut app, &mut renderer, WispEvent::Acp(event)).await? {
                            return Ok(());
                        }
                    }
                    None => return Ok(()),
                }
            }

            () = tick_fut => {
                if handle_event(&mut app, &mut renderer, WispEvent::Terminal(Event::Tick)).await? {
                    return Ok(());
                }
            }
        }
    }
}

async fn handle_event<W: io::Write>(
    app: &mut App,
    renderer: &mut Renderer<W>,
    event: WispEvent,
) -> Result<bool, Box<dyn std::error::Error>> {
    let ctx = renderer.context();
    let response = app.update(event, &ctx);
    if app.should_exit() {
        return Ok(true);
    }
    if process_effects(app, renderer, response).await? {
        return Ok(true);
    }
    renderer.render_frame(|ctx| app.view(ctx))?;
    Ok(false)
}

async fn process_effects<W: io::Write>(
    app: &mut App,
    renderer: &mut Renderer<W>,
    response: Option<Vec<AppAction>>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut queue: VecDeque<AppAction> = response.unwrap_or_default().into();

    while let Some(effect) = queue.pop_front() {
        renderer.render_frame(|ctx| app.view(ctx))?;
        let follow_up = app.run_effect(renderer, effect).await?;
        if app.should_exit() {
            return Ok(true);
        }
        queue.extend(follow_up);
    }

    Ok(false)
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
