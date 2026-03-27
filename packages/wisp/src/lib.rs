pub mod cli;
pub mod components;
pub mod error;
#[allow(dead_code)]
pub mod git_diff;
pub mod keybindings;
pub mod runtime_state;
pub mod settings;
#[cfg(test)]
pub(crate) mod test_helpers;

use components::app::App;
use error::AppError;
use runtime_state::RuntimeState;
use std::fs::create_dir_all;
use std::future::pending;
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio::{select, time};
use tracing_appender::rolling::daily;
use tracing_subscriber::EnvFilter;
use tui::{
    Component, CrosstermEvent, Event, MouseCapture, Renderer, RendererCommand, TerminalSession,
    spawn_terminal_event_task, terminal_size,
};

/// Launch the wisp TUI with the given agent subprocess command.
///
/// Sets up logging, connects to the agent via ACP, and runs the interactive
/// terminal event loop until the user exits.
pub async fn run_tui(agent_command: &str) -> Result<(), AppError> {
    setup_logging(None);
    let state = RuntimeState::new(agent_command).await?;
    run_with_state(state).await
}

/// Run the TUI from an already-initialized [`RuntimeState`].
pub async fn run_with_state(state: RuntimeState) -> Result<(), AppError> {
    let RuntimeState {
        session_id,
        agent_name,
        prompt_capabilities,
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
        prompt_capabilities,
        &config_options,
        auth_methods,
        working_dir,
        prompt_handle,
    );

    run_app(app, theme, event_rx).await
}

pub fn setup_logging(log_dir: Option<&str>) {
    let dir = log_dir.unwrap_or("/tmp/wisp-logs");
    create_dir_all(dir).ok();
    tracing_subscriber::fmt()
        .with_writer(daily(dir, "wisp.log"))
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}

fn render(renderer: &mut Renderer<impl io::Write>, app: &mut App) -> Result<(), AppError> {
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

    let mut last_mouse_capture = false;
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

        let capture = app.needs_mouse_capture();
        if last_mouse_capture != capture {
            renderer.apply_commands(vec![RendererCommand::SetMouseCapture(capture)])?;
            last_mouse_capture = capture;
        }
    }
}
