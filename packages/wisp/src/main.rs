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
use crate::components::app::attachments::build_attachment_blocks;
use crate::components::app::{App, AppMessage};
use crate::components::conversation_window::render_segments_to_lines;
use crate::error::AppError;
use crate::runtime_state::RuntimeState;
use crate::tui::advanced::{
    CrosstermEvent, MouseCapture, Renderer, TerminalSession, spawn_terminal_event_task,
    terminal_size,
};
use crate::tui::{Component, Event};
use acp_utils::client::AcpPromptHandle;
use clap::Parser;
use std::fs::create_dir_all;
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
        prompt_handle.clone(),
    );

    match run_app(app, prompt_handle, theme, event_rx).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fatal error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn render(renderer: &mut Renderer<impl io::Write>, app: &mut App) -> Result<(), AppError> {
    let context = renderer.context();
    app.prepare_for_render(&context);
    let app: &App = app;
    renderer.render_frame(|ctx| app.render(ctx))?;
    Ok(())
}

async fn process_messages(
    renderer: &mut Renderer<impl io::Write>,
    app: &mut App,
    prompt_handle: &AcpPromptHandle,
    messages: Vec<AppMessage>,
) -> Result<(), AppError> {
    for msg in messages {
        match msg {
            AppMessage::ClearScreen => renderer.clear_screen()?,
            AppMessage::SetTheme(theme) => renderer.set_theme(theme),
            AppMessage::PushToScrollbackContent {
                content,
                completed_tool_ids,
            } => {
                let context = renderer.context();
                let lines = render_segments_to_lines(&content, app.tool_call_statuses(), &context);
                if !lines.is_empty() {
                    renderer.push_to_scrollback(&lines)?;
                }
                app.remove_tools(&completed_tool_ids);
            }
            AppMessage::SendPrompt {
                user_input,
                attachments,
            } => {
                let echo = vec![
                    tui::Line::new(String::new()),
                    tui::Line::new(user_input.clone()),
                ];
                renderer.push_to_scrollback(&echo)?;

                let outcome = build_attachment_blocks(&attachments).await;
                if !outcome.warnings.is_empty() {
                    let lines: Vec<tui::Line> = outcome
                        .warnings
                        .into_iter()
                        .map(|w| tui::Line::new(format!("[wisp] {w}")))
                        .collect();
                    renderer.push_to_scrollback(&lines)?;
                }
                prompt_handle.prompt(
                    app.session_id(),
                    &user_input,
                    if outcome.blocks.is_empty() {
                        None
                    } else {
                        Some(outcome.blocks)
                    },
                )?;
            }
            AppMessage::LoadGitDiff | AppMessage::RefreshGitDiff => {
                app.git_diff_mode_mut().complete_load().await;
            }
        }
    }
    Ok(())
}

async fn run_app(
    mut app: App,
    prompt_handle: AcpPromptHandle,
    theme: tui::Theme,
    mut event_rx: mpsc::UnboundedReceiver<acp_utils::client::AcpEvent>,
) -> Result<(), AppError> {
    let size = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(io::stdout(), theme, size);
    let _session = TerminalSession::new(true, MouseCapture::Disabled)?;
    let mut terminal_rx = spawn_terminal_event_task();
    render(&mut renderer, &mut app)?;

    let tick_rate = Duration::from_millis(100);
    let mut tick_interval = {
        let mut tick = interval(tick_rate);
        tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        tick
    };

    loop {
        let tick_fut = async {
            if !app.wants_tick() {
                std::future::pending::<()>().await;
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
                    let messages = app.on_event(&tui_event).unwrap_or_default();
                    process_messages(&mut renderer, &mut app, &prompt_handle, messages).await?;
                    if app.exit_requested() { return Ok(()); }
                    render(&mut renderer, &mut app)?;
                }
            }

            app_event = event_rx.recv() => {
                match app_event {
                    Some(event) => {
                        let messages = app.on_acp_event(event);
                        process_messages(&mut renderer, &mut app, &prompt_handle, messages).await?;
                        if app.exit_requested() { return Ok(()); }
                        render(&mut renderer, &mut app)?;
                    }
                    None => return Ok(()),
                }
            }

            () = tick_fut => {
                let messages = app.on_event(&Event::Tick).unwrap_or_default();
                process_messages(&mut renderer, &mut app, &prompt_handle, messages).await?;
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
