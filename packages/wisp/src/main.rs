mod app_state;
mod cli;
mod components;
mod error;
mod tui;

use crate::app_state::AppState;
use crate::cli::Cli;
use crate::tui::Renderer;
use acp_utils::client::AcpEvent;
use agent_client_protocol as acp;
use clap::Parser;
use components::app::{App, AppEvent};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind, read};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::{select, time};

#[derive(Debug)]
enum TerminalEvent {
    Key(crossterm::event::KeyEvent),
    Paste(String),
    Resize(u16, u16),
}

#[derive(Debug, Clone)]
struct NonInteractiveThoughtState {
    in_block: bool,
    block_ends_with_newline: bool,
}

impl Default for NonInteractiveThoughtState {
    fn default() -> Self {
        Self {
            in_block: false,
            block_ends_with_newline: true,
        }
    }
}

impl NonInteractiveThoughtState {
    fn on_thought_chunk(&mut self, chunk: &str) -> Option<String> {
        if chunk.is_empty() {
            return None;
        }

        let mut output = String::new();
        if !self.in_block {
            output.push_str("Thought: ");
            self.in_block = true;
        }

        output.push_str(chunk);
        self.block_ends_with_newline = chunk.ends_with('\n');
        Some(output)
    }

    fn on_non_thought_update(&mut self) -> Option<&'static str> {
        if !self.in_block {
            return None;
        }

        self.in_block = false;
        if self.block_ends_with_newline {
            self.block_ends_with_newline = true;
            None
        } else {
            self.block_ends_with_newline = true;
            Some("\n")
        }
    }
}

fn spawn_terminal_event_task() -> mpsc::UnboundedReceiver<TerminalEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::task::spawn_blocking(move || {
        loop {
            let event = match read() {
                Ok(event) => event,
                Err(e) => {
                    eprintln!("Event read error: {e}");
                    continue;
                }
            };

            let terminal_event = match event {
                Event::Key(key_event) => Some(TerminalEvent::Key(key_event)),
                Event::Paste(text) => Some(TerminalEvent::Paste(text)),
                Event::Resize(cols, rows) => Some(TerminalEvent::Resize(cols, rows)),
                _ => None,
            };

            if let Some(event) = terminal_event
                && tx.send(event).is_err()
            {
                break;
            }
        }
    });
    rx
}

fn should_handle_key_event(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

fn apply_screen_effects<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &App,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &acp::SessionId,
    effects: Vec<AppEvent>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut should_render = false;

    for effect in effects {
        match effect {
            AppEvent::Exit => return Ok(true),
            AppEvent::Render => should_render = true,
            AppEvent::PushScrollback(lines) => renderer.push_to_scrollback(&lines)?,
            AppEvent::PromptSubmit {
                user_input,
                content_blocks,
            } => {
                prompt_handle.prompt(session_id, &user_input, content_blocks)?;
            }
            AppEvent::SetConfigOption {
                config_id,
                new_value,
            } => {
                let _ = prompt_handle.set_config_option(session_id, &config_id, &new_value);
            }
        }
    }

    if should_render {
        renderer.render(screen)?;
    }

    Ok(false)
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let log_dir = cli
        .log_dir
        .clone()
        .unwrap_or_else(|| "/tmp/wisp-logs".to_string());
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = tracing_appender::rolling::daily(&log_dir, "wisp.log");
    tracing_subscriber::fmt()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let state = match AppState::from_cli(&cli).await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Failed to initialize: {e}");
            return ExitCode::FAILURE;
        }
    };

    let result = if cli.prompt.is_empty() {
        run_terminal_ui(state).await.map(|_| ExitCode::SUCCESS)
    } else {
        let prompt = cli.prompt.join(" ");
        run_non_interactive(state, &prompt).await
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Fatal error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run_terminal_ui(mut state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnableBracketedPaste)?;
    let mut screen = App::new(state.agent_name.clone(), &state.config_options);
    let mut renderer = Renderer::new(io::stdout());

    renderer.update_render_context();
    renderer.render(&screen)?;
    let mut terminal_event_rx = spawn_terminal_event_task();
    let mut animation_interval = time::interval(Duration::from_millis(16));
    animation_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        select! {
            Some(event) = state.event_rx.recv() => {
                match event {
                    AcpEvent::SessionUpdate(update) => {
                        let effects = screen.on_session_update(*update);
                        if let Err(e) = apply_screen_effects(
                            &mut renderer,
                            &screen,
                            &state.prompt_handle,
                            &state.session_id,
                            effects,
                        ) {
                            eprintln!("Error handling session update: {e}");
                        }
                    }
                    AcpEvent::ExtNotification(notification) => {
                        let effects = screen.on_ext_notification(notification);
                        if let Err(e) = apply_screen_effects(
                            &mut renderer,
                            &screen,
                            &state.prompt_handle,
                            &state.session_id,
                            effects,
                        ) {
                            eprintln!("Error handling ext notification: {e}");
                        }
                    }
                    AcpEvent::PromptDone(_stop_reason) => {
                        let effects = screen.on_prompt_done(renderer.context().size);
                        if let Err(e) = apply_screen_effects(
                            &mut renderer,
                            &screen,
                            &state.prompt_handle,
                            &state.session_id,
                            effects,
                        ) {
                            eprintln!("Error handling prompt done: {e}");
                        }
                    }
                    AcpEvent::PromptError(e) => {
                        let effects = screen.on_prompt_error();
                        if let Err(render_err) = apply_screen_effects(
                            &mut renderer,
                            &screen,
                            &state.prompt_handle,
                            &state.session_id,
                            effects,
                        ) {
                            eprintln!("Error handling prompt error render: {render_err}");
                        }
                        eprintln!("Prompt error: {e}");
                    }
                    AcpEvent::ConnectionClosed => {
                        break;
                    }
                }
            }

            Some(terminal_event) = terminal_event_rx.recv() => {
                match terminal_event {
                    TerminalEvent::Key(key_event) => {
                        if should_handle_key_event(key_event.kind) {
                            let effects = screen.on_key_event(key_event);
                            match apply_screen_effects(
                                &mut renderer,
                                &screen,
                                &state.prompt_handle,
                                &state.session_id,
                                effects,
                            ) {
                                Ok(true) => break,
                                Ok(false) => {}
                                Err(err) => {
                                    eprintln!("Error handling key event: {err}");
                                }
                            }
                        }
                    }
                    TerminalEvent::Paste(text) => {
                        let effects = screen.on_paste(&text);
                        if let Err(e) = apply_screen_effects(
                            &mut renderer,
                            &screen,
                            &state.prompt_handle,
                            &state.session_id,
                            effects,
                        ) {
                            eprintln!("Error handling paste: {e}");
                        }
                    }
                    TerminalEvent::Resize(cols, rows) => {
                        renderer.update_render_context_with((cols, rows));
                        let effects = screen.on_resize(cols, rows);
                        if let Err(e) = apply_screen_effects(
                            &mut renderer,
                            &screen,
                            &state.prompt_handle,
                            &state.session_id,
                            effects,
                        ) {
                            eprintln!("Error handling resize: {e}");
                        }
                    }
                }
            }

            _ = animation_interval.tick() => {
                let effects = screen.on_tick();
                if let Err(e) = apply_screen_effects(
                    &mut renderer,
                    &screen,
                    &state.prompt_handle,
                    &state.session_id,
                    effects,
                ) {
                    eprintln!("Error on tick: {e}");
                }
            }
        }
    }

    crossterm::execute!(io::stdout(), DisableBracketedPaste)?;
    disable_raw_mode()?;
    println!("\nGoodbye!");
    Ok(())
}

async fn run_non_interactive(
    mut state: AppState,
    prompt: &str,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut thought_state = NonInteractiveThoughtState::default();

    state
        .prompt_handle
        .prompt(&state.session_id, prompt, None)?;

    while let Some(event) = state.event_rx.recv().await {
        match event {
            AcpEvent::SessionUpdate(update) => match *update {
                acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                    if let acp::ContentBlock::Text(text_content) = chunk.content
                        && let Some(output) = thought_state.on_thought_chunk(&text_content.text)
                    {
                        print!("{output}");
                        io::stdout().flush()?;
                    }
                }

                non_thought_update => {
                    if let Some(output) = thought_state.on_non_thought_update() {
                        print!("{output}");
                        io::stdout().flush()?;
                    }

                    match non_thought_update {
                        acp::SessionUpdate::AgentMessageChunk(chunk) => {
                            if let acp::ContentBlock::Text(text_content) = chunk.content {
                                print!("{}", text_content.text);
                                io::stdout().flush()?;
                            }
                        }
                        acp::SessionUpdate::ToolCall(tool_call) => {
                            println!("[Tool: {}] Starting...", tool_call.title);
                        }
                        acp::SessionUpdate::ToolCallUpdate(update) => {
                            if let Some(status) = &update.fields.status {
                                match status {
                                    acp::ToolCallStatus::Completed => {
                                        println!("[Tool: {}] ✓ Completed", update.tool_call_id);
                                    }
                                    acp::ToolCallStatus::Failed => {
                                        eprintln!("[Tool: {}] ✗ Failed", update.tool_call_id);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            },
            AcpEvent::ExtNotification(_) => {}
            AcpEvent::PromptDone(_) => {
                if let Some(output) = thought_state.on_non_thought_update() {
                    print!("{output}");
                    io::stdout().flush()?;
                }
                println!();
                break;
            }
            AcpEvent::PromptError(e) => {
                if let Some(output) = thought_state.on_non_thought_update() {
                    print!("{output}");
                    io::stdout().flush()?;
                }
                eprintln!("Error: {e}");
                return Ok(ExitCode::FAILURE);
            }
            AcpEvent::ConnectionClosed => {
                if let Some(output) = thought_state.on_non_thought_update() {
                    print!("{output}");
                    io::stdout().flush()?;
                }
                break;
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::{NonInteractiveThoughtState, should_handle_key_event};
    use crossterm::event::KeyEventKind;

    #[test]
    fn non_interactive_prefixes_once_per_contiguous_block() {
        let mut state = NonInteractiveThoughtState::default();
        assert_eq!(
            state.on_thought_chunk("plan"),
            Some("Thought: plan".to_string())
        );
        assert_eq!(state.on_thought_chunk(" more"), Some(" more".to_string()));
        assert_eq!(state.on_non_thought_update(), Some("\n"));
        assert_eq!(
            state.on_thought_chunk("next"),
            Some("Thought: next".to_string())
        );
    }

    #[test]
    fn non_interactive_does_not_emit_extra_newline_when_chunk_ends_with_newline() {
        let mut state = NonInteractiveThoughtState::default();
        assert_eq!(
            state.on_thought_chunk("plan\n"),
            Some("Thought: plan\n".to_string())
        );
        assert_eq!(state.on_non_thought_update(), None);
    }

    #[test]
    fn handles_press_and_repeat_key_events() {
        assert!(should_handle_key_event(KeyEventKind::Press));
        assert!(should_handle_key_event(KeyEventKind::Repeat));
        assert!(!should_handle_key_event(KeyEventKind::Release));
    }
}
