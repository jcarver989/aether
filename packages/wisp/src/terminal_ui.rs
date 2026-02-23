use crate::app_state::AppState;
use crate::components::app::{App, AppEvent, build_attachment_blocks};
use crate::tui::{Line, Renderer, spawn_terminal_event_task};
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
                // Combine additional ready ACP events into single render pass.
                let mut events = collect_acp_events(&mut screen, &renderer, event);
                while let Ok(event) = event_rx.try_recv() {
                    events.extend(collect_acp_events(&mut screen, &renderer, event));
                }

                match apply_screen_effects(&mut renderer, &mut screen, &prompt_handle, &session_id, events).await {
                    Ok(true) => break,
                    Err(e) => eprintln!("Error handling ACP event: {e}"),
                    _ => {}
                }
            }

            Some(terminal_event) = terminal_event_rx.recv() => {
                if on_terminal_event(
                    &mut renderer,
                    &mut screen,
                    &prompt_handle,
                    &session_id,
                    terminal_event,
                )
                .await
                {
                    break;
                }
            }

            _ = animation_interval.tick() => {
                on_tick(
                    &mut renderer,
                    &mut screen,
                    &prompt_handle,
                    &session_id,
                )
                .await;
            }
        }
    }

    crossterm::execute!(io::stdout(), DisableBracketedPaste)?;
    disable_raw_mode()?;
    println!("\nGoodbye!");
    Ok(())
}

/// Extract effects from an ACP event without rendering. Multiple events can be
/// batched and their effects applied together in a single render pass.
fn collect_acp_events<T: Write>(
    screen: &mut App,
    renderer: &Renderer<T>,
    event: AcpEvent,
) -> Vec<AppEvent> {
    match event {
        AcpEvent::SessionUpdate(update) => screen.on_session_update(*update),
        AcpEvent::ExtNotification(notification) => screen.on_ext_notification(&notification),
        AcpEvent::PromptDone(_) => screen.on_prompt_done(renderer.context().size),
        AcpEvent::PromptError(e) => {
            eprintln!("Prompt error: {e}");
            screen.on_prompt_error()
        }
        AcpEvent::ElicitationRequest {
            params,
            response_tx,
        } => screen.on_elicitation_request(params, response_tx),
        AcpEvent::ConnectionClosed => vec![AppEvent::Exit],
    }
}

async fn on_terminal_event<T: Write>(
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
                match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects)
                    .await
                {
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
            match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects).await {
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
            match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects).await {
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

async fn on_tick<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &mut App,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &acp::SessionId,
) {
    let effects = screen.on_tick();
    if let Err(e) = apply_screen_effects(renderer, screen, prompt_handle, session_id, effects).await
    {
        eprintln!("Error on tick: {e}");
    }
}

fn should_handle_key_event(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

async fn apply_screen_effects<T: Write>(
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
                attachments,
            } => {
                submit_prompt_with_attachments(
                    renderer,
                    screen,
                    prompt_handle,
                    session_id,
                    &user_input,
                    attachments,
                    &mut should_render,
                )
                .await?;
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

async fn submit_prompt_with_attachments<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &mut App,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &acp::SessionId,
    user_input: &str,
    attachments: Vec<crate::components::app::PromptAttachment>,
    should_render: &mut bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if *should_render {
        renderer.render(screen)?;
        *should_render = false;
    }

    let outcome = build_attachment_blocks(&attachments).await;
    if !outcome.warnings.is_empty() {
        let warning_lines: Vec<Line> = outcome
            .warnings
            .into_iter()
            .map(|warning| Line::new(format!("[wisp] {warning}")))
            .collect();
        renderer.push_to_scrollback(&warning_lines)?;
        *should_render = true;
    }

    prompt_handle.prompt(
        session_id,
        user_input,
        if outcome.blocks.is_empty() {
            None
        } else {
            Some(outcome.blocks)
        },
    )?;

    Ok(())
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
