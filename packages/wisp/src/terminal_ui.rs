use crate::app_state::AppState;
use crate::components::app::{App, AppEvent};
use crate::tui::{Renderer, spawn_terminal_event_task};
use acp_utils::client::AcpEvent;
use agent_client_protocol as acp;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{self, Write};
use std::time::Duration;
use tokio::{select, time};

pub(crate) async fn run_terminal_ui(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let AppState {
        session_id,
        agent_name,
        config_options,
        mut event_rx,
        prompt_handle,
    } = state;

    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnableBracketedPaste)?;

    let mut screen = App::new(agent_name, &config_options);
    let mut renderer = Renderer::new(io::stdout());
    renderer.update_render_context();
    renderer.render(&mut screen)?;

    let mut terminal_event_rx = spawn_terminal_event_task();
    let mut animation_interval = time::interval(Duration::from_millis(100));
    animation_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        select! {
            Some(event) = event_rx.recv() => {
                if on_acp_event(
                    &mut renderer,
                    &mut screen,
                    &prompt_handle,
                    &session_id,
                    event,
                ) {
                    break;
                }
            }

            Some(terminal_event) = terminal_event_rx.recv() => {
                if on_terminal_event(
                    &mut renderer,
                    &mut screen,
                    &prompt_handle,
                    &session_id,
                    terminal_event,
                ) {
                    break;
                }
            }

            _ = animation_interval.tick() => {
                on_tick(
                    &mut renderer,
                    &mut screen,
                    &prompt_handle,
                    &session_id,
                );
            }
        }
    }

    crossterm::execute!(io::stdout(), DisableBracketedPaste)?;
    disable_raw_mode()?;
    println!("\nGoodbye!");
    Ok(())
}

fn on_acp_event<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &mut App,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &acp::SessionId,
    event: AcpEvent,
) -> bool {
    match event {
        AcpEvent::SessionUpdate(update) => {
            let effects = screen.on_session_update(*update);
            match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects) {
                Ok(true) => true,
                Ok(false) => false,
                Err(e) => {
                    eprintln!("Error handling session update: {e}");
                    false
                }
            }
        }
        AcpEvent::ExtNotification(notification) => {
            let effects = screen.on_ext_notification(&notification);
            match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects) {
                Ok(true) => true,
                Ok(false) => false,
                Err(e) => {
                    eprintln!("Error handling ext notification: {e}");
                    false
                }
            }
        }
        AcpEvent::PromptDone(_) => {
            let effects = screen.on_prompt_done(renderer.context().size);
            match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects) {
                Ok(true) => true,
                Ok(false) => false,
                Err(e) => {
                    eprintln!("Error handling prompt done: {e}");
                    false
                }
            }
        }
        AcpEvent::PromptError(e) => {
            let effects = screen.on_prompt_error();
            if let Err(render_err) =
                apply_screen_effects(renderer, screen, prompt_handle, session_id, effects)
            {
                eprintln!("Error handling prompt error render: {render_err}");
            }
            eprintln!("Prompt error: {e}");
            false
        }
        AcpEvent::ConnectionClosed => true,
    }
}

fn on_terminal_event<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &mut App,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &acp::SessionId,
    terminal_event: Event,
) -> bool {
    match terminal_event {
        Event::Key(key_event) => {
            if should_handle_key_event(key_event.kind) {
                let effects = screen.on_key_event(key_event);
                match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects) {
                    Ok(true) => true,
                    Ok(false) => false,
                    Err(err) => {
                        eprintln!("Error handling key event: {err}");
                        false
                    }
                }
            } else {
                false
            }
        }
        Event::Paste(text) => {
            let effects = screen.on_paste(&text);
            match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects) {
                Ok(true) => true,
                Ok(false) => false,
                Err(e) => {
                    eprintln!("Error handling paste: {e}");
                    false
                }
            }
        }
        Event::Resize(cols, rows) => {
            renderer.update_render_context_with((cols, rows));
            let effects = App::on_resize(cols, rows);
            match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects) {
                Ok(true) => true,
                Ok(false) => false,
                Err(e) => {
                    eprintln!("Error handling resize: {e}");
                    false
                }
            }
        }
        _ => false,
    }
}

fn on_tick<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &mut App,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &acp::SessionId,
) {
    let effects = screen.on_tick();
    if let Err(e) = apply_screen_effects(renderer, screen, prompt_handle, session_id, effects) {
        eprintln!("Error on tick: {e}");
    }
}

fn should_handle_key_event(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

fn apply_screen_effects<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &mut App,
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
            AppEvent::Cancel => {
                prompt_handle.cancel(session_id)?;
                should_render = true;
            }
        }
    }

    if should_render {
        renderer.render(screen)?;
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::should_handle_key_event;
    use crossterm::event::KeyEventKind;

    #[test]
    fn handles_press_and_repeat_key_events() {
        assert!(should_handle_key_event(KeyEventKind::Press));
        assert!(should_handle_key_event(KeyEventKind::Repeat));
        assert!(!should_handle_key_event(KeyEventKind::Release));
    }
}
