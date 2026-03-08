use super::{App, AppEffect, AppRuntimeAction, PromptAttachment, build_attachment_blocks};
use crate::settings::{load_or_create_settings, save_settings};
use crate::tui::{Line, Renderer, RuntimeAction};
use acp_utils::client::AcpPromptHandle;
use agent_client_protocol as acp;
use crossterm::event::KeyEventKind;
use std::io::{self, Write};

#[derive(Clone, Copy)]
pub struct PromptContext<'a> {
    pub prompt_handle: &'a AcpPromptHandle,
    pub session_id: &'a acp::SessionId,
}

impl<'a> PromptContext<'a> {
    pub fn new(prompt_handle: &'a AcpPromptHandle, session_id: &'a acp::SessionId) -> Self {
        Self {
            prompt_handle,
            session_id,
        }
    }
}

pub fn should_handle_key_event(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

pub fn apply_theme_selection<T: Write>(renderer: &mut Renderer<T>, file: Option<String>) {
    let mut settings = load_or_create_settings();
    settings.theme.file = file;

    if let Err(err) = save_settings(&settings) {
        tracing::warn!("Failed to persist theme setting: {err}");
    }

    let theme = crate::settings::load_theme(&settings);
    renderer.set_theme(theme);
}

pub async fn submit_prompt_with_attachments<T: Write>(
    renderer: &mut Renderer<T>,
    prompt: PromptContext<'_>,
    user_input: &str,
    attachments: Vec<PromptAttachment>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let outcome = build_attachment_blocks(&attachments).await;
    let should_render = !outcome.warnings.is_empty();

    if should_render {
        let warning_lines: Vec<Line> = outcome
            .warnings
            .into_iter()
            .map(|warning| Line::new(format!("[wisp] {warning}")))
            .collect();
        renderer.push_to_scrollback(&warning_lines)?;
    }

    prompt.prompt_handle.prompt(
        prompt.session_id,
        user_input,
        if outcome.blocks.is_empty() {
            None
        } else {
            Some(outcome.blocks)
        },
    )?;

    Ok(should_render)
}

pub async fn apply_app_effect<T: Write>(
    app: &mut App,
    renderer: &mut Renderer<T>,
    effect: AppEffect,
    prompt: Option<PromptContext<'_>>,
) -> Result<Vec<AppRuntimeAction>, Box<dyn std::error::Error>> {
    match effect {
        AppEffect::PushScrollback(lines) => {
            renderer.push_to_scrollback(&lines)?;
            Ok(vec![])
        }
        AppEffect::PromptSubmit {
            user_input,
            attachments,
        } => {
            let should_render = submit_prompt_with_attachments(
                renderer,
                required_prompt_context(prompt)?,
                &user_input,
                attachments,
            )
            .await?;
            Ok(if should_render {
                vec![RuntimeAction::Render]
            } else {
                vec![]
            })
        }
        AppEffect::SetConfigOption {
            config_id,
            new_value,
        } => {
            let prompt = required_prompt_context(prompt)?;
            let _ =
                prompt
                    .prompt_handle
                    .set_config_option(prompt.session_id, &config_id, &new_value);
            Ok(vec![])
        }
        AppEffect::SetTheme { file } => {
            apply_theme_selection(renderer, file);
            Ok(vec![RuntimeAction::Render])
        }
        AppEffect::Cancel => {
            let prompt = required_prompt_context(prompt)?;
            prompt.prompt_handle.cancel(prompt.session_id)?;
            Ok(vec![RuntimeAction::Render])
        }
        AppEffect::AuthenticateMcpServer { server_name } => {
            let prompt = required_prompt_context(prompt)?;
            let _ = prompt
                .prompt_handle
                .authenticate_mcp_server(prompt.session_id, &server_name);
            Ok(vec![RuntimeAction::Render])
        }
        AppEffect::AuthenticateProvider { method_id } => {
            let prompt = required_prompt_context(prompt)?;
            let _ = prompt
                .prompt_handle
                .authenticate(prompt.session_id, &method_id);
            app.on_authenticate_started(&method_id);
            Ok(vec![RuntimeAction::Render])
        }
    }
}

fn required_prompt_context(prompt: Option<PromptContext<'_>>) -> io::Result<PromptContext<'_>> {
    prompt.ok_or_else(|| io::Error::other("missing prompt context"))
}

#[cfg(test)]
mod tests {
    use super::{
        PromptContext, apply_theme_selection, should_handle_key_event,
        submit_prompt_with_attachments,
    };
    use crate::settings::{ThemeSettings, WispSettings, load_or_create_settings, save_settings};
    use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
    use crate::tui::Renderer;
    use crate::tui::theme::Theme;
    use acp_utils::client::AcpPromptHandle;
    use agent_client_protocol as acp;
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
    fn apply_theme_selection_persists_default_theme_as_none() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        with_wisp_home(temp_dir.path(), || {
            save_settings(&WispSettings {
                theme: ThemeSettings {
                    file: Some("old.tmTheme".to_string()),
                },
            })
            .unwrap();

            let mut renderer = Renderer::new(Vec::new(), Theme::default());
            apply_theme_selection(&mut renderer, None);

            let loaded = load_or_create_settings();
            assert_eq!(loaded.theme.file, None);
        });
    }

    #[tokio::test]
    async fn submit_prompt_with_attachments_reports_warning_render_need() {
        let mut renderer = Renderer::new(Vec::new(), Theme::default());
        let prompt_handle = AcpPromptHandle::noop();
        let session_id = acp::SessionId::new("test");
        let attachment = crate::components::app::PromptAttachment {
            path: std::path::PathBuf::from("missing-file.txt"),
            display_name: "missing-file.txt".to_string(),
        };

        let should_render = submit_prompt_with_attachments(
            &mut renderer,
            PromptContext::new(&prompt_handle, &session_id),
            "hello",
            vec![attachment],
        )
        .await
        .unwrap();

        assert!(should_render);
    }
}
