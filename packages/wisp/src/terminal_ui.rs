use crate::components::app::{App, AppAction, AppEffect, build_attachment_blocks};
use crate::runtime_state::RuntimeState;
use crate::settings::{load_or_create_settings, save_settings};
use crate::tui::{Line, Renderer, spawn_terminal_event_task};
use acp_utils::client::AcpEvent;
use agent_client_protocol as acp;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{self, Write};
use std::time::Duration;
use tokio::{select, time};

pub(crate) async fn run_terminal_ui(state: RuntimeState) -> Result<(), Box<dyn std::error::Error>> {
    let RuntimeState {
        session_id,
        agent_name,
        config_options,
        auth_methods,
        theme,
        mut event_rx,
        prompt_handle,
    } = state;

    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnableBracketedPaste)?;

    let mut screen = App::new(agent_name, &config_options, auth_methods);
    let mut renderer = Renderer::new(io::stdout(), theme);
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

/// Map an ACP event to an action and dispatch it. Multiple events can be
/// batched and their effects applied together in a single render pass.
fn collect_acp_events<T: Write>(
    screen: &mut App,
    renderer: &Renderer<T>,
    event: AcpEvent,
) -> Vec<AppEffect> {
    let action = match event {
        AcpEvent::SessionUpdate(update) => AppAction::SessionUpdate(*update),
        AcpEvent::ExtNotification(notification) => AppAction::ExtNotification(notification),
        AcpEvent::PromptDone(_) => AppAction::PromptDone,
        AcpEvent::PromptError(e) => {
            eprintln!("Prompt error: {e}");
            AppAction::PromptError
        }
        AcpEvent::ElicitationRequest {
            params,
            response_tx,
        } => AppAction::ElicitationRequest {
            params,
            response_tx,
        },
        AcpEvent::AuthenticateComplete { method_id } => {
            AppAction::AuthenticateComplete { method_id }
        }
        AcpEvent::AuthenticateFailed { method_id, error } => {
            AppAction::AuthenticateFailed { method_id, error }
        }
        AcpEvent::ConnectionClosed => return vec![AppEffect::Exit],
    };
    screen.dispatch(action, renderer.context())
}

async fn on_terminal_event<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &mut App,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &acp::SessionId,
    terminal_event: Event,
) -> bool {
    let effects = match terminal_event {
        Event::Key(key_event) if should_handle_key_event(key_event.kind) => {
            screen.dispatch(AppAction::Key(key_event), renderer.context())
        }
        Event::Paste(text) => screen.dispatch(AppAction::Paste(text), renderer.context()),
        Event::Resize(cols, rows) => {
            renderer.update_render_context_with((cols, rows));
            screen.dispatch(AppAction::Resize { cols, rows }, renderer.context())
        }
        _ => return false,
    };

    match apply_screen_effects(renderer, screen, prompt_handle, session_id, effects).await {
        Ok(true) => true,
        Ok(false) => false,
        Err(e) => {
            eprintln!("Error handling terminal event: {e}");
            false
        }
    }
}

async fn on_tick<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &mut App,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &acp::SessionId,
) {
    let effects = screen.dispatch(AppAction::Tick, renderer.context());
    if let Err(e) = apply_screen_effects(renderer, screen, prompt_handle, session_id, effects).await
    {
        eprintln!("Error on tick: {e}");
    }
}

fn should_handle_key_event(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

fn apply_theme_selection<T: Write>(renderer: &mut Renderer<T>, file: Option<String>) {
    let mut settings = load_or_create_settings();
    settings.theme.file = file;

    if let Err(err) = save_settings(&settings) {
        tracing::warn!("Failed to persist theme setting: {err}");
    }

    let theme = crate::settings::load_theme(&settings);
    renderer.set_theme(theme);
}

async fn apply_screen_effects<T: Write>(
    renderer: &mut Renderer<T>,
    screen: &mut App,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &acp::SessionId,
    effects: Vec<AppEffect>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut should_render = false;

    for effect in effects {
        match effect {
            AppEffect::Exit => return Ok(true),
            AppEffect::Render => should_render = true,
            AppEffect::PushScrollback(lines) => {
                if should_render {
                    renderer.render(screen)?;
                    should_render = false;
                }
                renderer.push_to_scrollback(&lines)?;
            }
            AppEffect::PromptSubmit {
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
            AppEffect::SetConfigOption {
                config_id,
                new_value,
            } => {
                let _ = prompt_handle.set_config_option(session_id, &config_id, &new_value);
            }
            AppEffect::SetTheme { file } => {
                apply_theme_selection(renderer, file);
                should_render = true;
            }
            AppEffect::Cancel => {
                prompt_handle.cancel(session_id)?;
                should_render = true;
            }
            AppEffect::AuthenticateMcpServer { server_name } => {
                let _ = prompt_handle.authenticate_mcp_server(session_id, &server_name);
                should_render = true;
            }
            AppEffect::AuthenticateProvider { method_id } => {
                let _ = prompt_handle.authenticate(session_id, &method_id);
                screen.on_authenticate_started(&method_id);
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
    use super::{apply_theme_selection, should_handle_key_event};
    use crate::tui::theme::Theme;
    use crate::settings::{ThemeSettings, WispSettings, load_or_create_settings, save_settings};
    use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
    use crate::tui::Renderer;
    use crossterm::event::KeyEventKind;
    use crossterm::style::Color;

    #[test]
    fn handles_press_and_repeat_key_events() {
        assert!(should_handle_key_event(KeyEventKind::Press));
        assert!(should_handle_key_event(KeyEventKind::Repeat));
        assert!(!should_handle_key_event(KeyEventKind::Release));
    }

    #[test]
    fn apply_theme_selection_persists_and_applies_theme_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("custom.tmTheme"), CUSTOM_TMTHEME).unwrap();

        with_wisp_home(temp_dir.path(), || {
            let mut renderer = Renderer::new(Vec::new(), Theme::default());
            apply_theme_selection(&mut renderer, Some("custom.tmTheme".to_string()));

            assert_eq!(
                renderer.context().theme.text_primary(),
                Color::Rgb {
                    r: 0x11,
                    g: 0x22,
                    b: 0x33
                }
            );

            let loaded = load_or_create_settings();
            assert_eq!(loaded.theme.file.as_deref(), Some("custom.tmTheme"));
        });
    }

    #[test]
    fn apply_theme_selection_default_clears_persisted_theme() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        with_wisp_home(temp_dir.path(), || {
            save_settings(&WispSettings {
                theme: ThemeSettings {
                    file: Some("custom.tmTheme".to_string()),
                },
            })
            .unwrap();

            let mut renderer = Renderer::new(Vec::new(), Theme::default());
            apply_theme_selection(&mut renderer, None);

            let loaded = load_or_create_settings();
            assert_eq!(loaded.theme.file, None);
        });
    }
}
