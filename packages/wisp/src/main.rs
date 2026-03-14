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
use crate::components::app::view::build_frame;
use crate::components::app::{UiState, UiStateController, ViewEffect, WispEvent};
use crate::components::conversation_window::render_segments_to_lines;
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

    let ui_state = UiState::new(agent_name, &config_options, auth_methods, working_dir);
    let controller = UiStateController::new(session_id, prompt_handle);

    match run_app(ui_state, controller, theme, event_rx).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fatal error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn apply_effects(
    renderer: &mut Renderer<impl io::Write>,
    state: &mut UiState,
    effects: Vec<ViewEffect>,
) -> Result<(), AppError> {
    for effect in effects {
        match effect {
            ViewEffect::ClearScreen => renderer.clear_screen()?,
            ViewEffect::SetTheme(theme) => renderer.set_theme(theme),
            ViewEffect::PushToScrollbackContent { content, completed_tool_ids } => {
                let context = renderer.context();
                let lines = render_segments_to_lines(&content, &state.tool_call_statuses, &context);
                if !lines.is_empty() {
                    renderer.push_to_scrollback(&lines)?;
                }
                state.remove_tools(&completed_tool_ids);
            }
            ViewEffect::PromptSubmitted { user_input } => {
                let lines = vec![
                    tui::Line::new(String::new()),
                    tui::Line::new(user_input),
                ];
                renderer.push_to_scrollback(&lines)?;
            }
            ViewEffect::AttachmentWarnings(warnings) => {
                let lines: Vec<tui::Line> = warnings
                    .into_iter()
                    .map(|w| tui::Line::new(format!("[wisp] {w}")))
                    .collect();
                renderer.push_to_scrollback(&lines)?;
            }
        }
    }
    Ok(())
}

fn render(renderer: &mut Renderer<impl io::Write>, state: &mut UiState) -> Result<(), AppError> {
    let context = renderer.context();
    state.prepare_for_render(&context);
    let state: &UiState = state;
    renderer.render_frame(|ctx| {
        build_frame(state, &state.git_diff_mode, ctx)
    })?;
    Ok(())
}

async fn run_app(
    mut state: UiState,
    mut controller: UiStateController,
    theme: tui::Theme,
    mut event_rx: mpsc::UnboundedReceiver<acp_utils::client::AcpEvent>,
) -> Result<(), AppError> {
    let _session = TerminalSession::enter(true, MouseCapture::Disabled)?;
    let mut renderer = Renderer::new(io::stdout(), theme);
    let size = terminal_size().unwrap_or((80, 24));
    renderer.on_resize(size);

    let mut terminal_rx = spawn_terminal_event_task();
    render(&mut renderer, &mut state)?;

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
                    renderer.on_resize((*cols, *rows));
                }
                if let Ok(tui_event) = Event::try_from(event) {
                    let effects = controller.handle_event(&mut state, WispEvent::Terminal(tui_event)).await?;
                    apply_effects(&mut renderer, &mut state, effects)?;
                    if state.exit_requested { return Ok(()); }
                    render(&mut renderer, &mut state)?;
                }
            }

            app_event = external_fut => {
                match app_event {
                    Some(event) => {
                        let effects = controller.handle_event(&mut state, WispEvent::Acp(event)).await?;
                        apply_effects(&mut renderer, &mut state, effects)?;
                        if state.exit_requested { return Ok(()); }
                        render(&mut renderer, &mut state)?;
                    }
                    None => return Ok(()),
                }
            }

            () = tick_fut => {
                let effects = controller.handle_event(&mut state, WispEvent::Terminal(Event::Tick)).await?;
                apply_effects(&mut renderer, &mut state, effects)?;
                if state.exit_requested { return Ok(()); }
                render(&mut renderer, &mut state)?;
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
