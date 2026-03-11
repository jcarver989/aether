use super::{App, AppAction, PromptAttachment, build_attachment_blocks};
use crate::components::app::git_diff_mode::format_review_prompt;
use crate::settings::{load_or_create_settings, save_settings};
use crate::tui::Line;
use crate::tui::advanced::Renderer;
use std::io::Write;

impl App {
    pub async fn apply_action(
        &mut self,
        terminal: &mut Renderer<impl Write>,
        action: AppAction,
    ) -> Result<Vec<AppAction>, Box<dyn std::error::Error>> {
        match action {
            AppAction::PushScrollback(lines) => {
                terminal.push_to_scrollback(&lines)?;
                Ok(vec![])
            }
            AppAction::PromptSubmit {
                user_input,
                attachments,
            } => {
                submit_prompt_with_attachments(
                    terminal,
                    &self.prompt_handle,
                    &self.session_id,
                    &user_input,
                    attachments,
                )
                .await?;
                Ok(vec![])
            }
            AppAction::SetConfigOption {
                config_id,
                new_value,
            } => {
                let _ =
                    self.prompt_handle
                        .set_config_option(&self.session_id, &config_id, &new_value);
                Ok(vec![])
            }
            AppAction::SetTheme { file } => {
                apply_theme_selection(terminal, file);
                Ok(vec![])
            }
            AppAction::Cancel => {
                self.prompt_handle.cancel(&self.session_id)?;
                Ok(vec![])
            }
            AppAction::AuthenticateMcpServer { server_name } => {
                let _ = self
                    .prompt_handle
                    .authenticate_mcp_server(&self.session_id, &server_name);
                Ok(vec![])
            }
            AppAction::AuthenticateProvider { method_id } => {
                let _ = self
                    .prompt_handle
                    .authenticate(&self.session_id, &method_id);
                self.state.on_authenticate_started(&method_id);
                Ok(vec![])
            }
            AppAction::ClearScreen => {
                self.state.reset_after_context_cleared();
                terminal.clear_screen()?;
                self.prompt_handle
                    .prompt(&self.session_id, "/clear", None)?;
                Ok(vec![])
            }
            AppAction::OpenGitDiffViewer => {
                self.state.enter_git_diff();
                self.git_diff_mode.begin_open();
                self.git_diff_mode.complete_load().await;
                Ok(vec![])
            }
            AppAction::RefreshGitDiffViewer => {
                self.git_diff_mode.complete_load().await;
                Ok(vec![])
            }
            AppAction::CloseGitDiffViewer => {
                self.git_diff_mode.close();
                self.state.exit_git_diff();
                Ok(vec![])
            }
            AppAction::SubmitDiffReview { comments } => {
                let prompt = format_review_prompt(&comments);
                self.git_diff_mode.close();
                self.state.exit_git_diff();
                submit_prompt_with_attachments(
                    terminal,
                    &self.prompt_handle,
                    &self.session_id,
                    &prompt,
                    vec![],
                )
                .await?;
                Ok(vec![])
            }
        }
    }
}

pub fn apply_theme_selection(terminal: &mut Renderer<impl Write>, file: Option<String>) {
    let mut settings = load_or_create_settings();
    settings.theme.file = file;

    if let Err(err) = save_settings(&settings) {
        tracing::warn!("Failed to persist theme setting: {err}");
    }

    let theme = crate::settings::load_theme(&settings);
    terminal.set_theme(theme);
}

pub async fn submit_prompt_with_attachments(
    terminal: &mut Renderer<impl Write>,
    prompt_handle: &acp_utils::client::AcpPromptHandle,
    session_id: &agent_client_protocol::SessionId,
    user_input: &str,
    attachments: Vec<PromptAttachment>,
) -> Result<(), Box<dyn std::error::Error>> {
    let outcome = build_attachment_blocks(&attachments).await;

    if !outcome.warnings.is_empty() {
        let warning_lines: Vec<Line> = outcome
            .warnings
            .into_iter()
            .map(|warning| Line::new(format!("[wisp] {warning}")))
            .collect();
        terminal.push_to_scrollback(&warning_lines)?;
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
    use super::{apply_theme_selection, submit_prompt_with_attachments};
    use crate::settings::{ThemeSettings, WispSettings, load_or_create_settings, save_settings};
    use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
    use crate::tui::advanced::Renderer;
    use crate::tui::{Color, Theme};
    use acp_utils::client::AcpPromptHandle;
    use agent_client_protocol as acp;

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
    async fn clear_screen_sends_clear_prompt_to_agent() {
        use crate::components::app::App;
        use std::path::PathBuf;

        let mut renderer = Renderer::new(Vec::new(), Theme::default());
        let prompt_handle = AcpPromptHandle::noop();
        let session_id = acp::SessionId::new("test");
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            prompt_handle,
            session_id,
            PathBuf::from("."),
        );

        let _effects = app
            .apply_action(
                &mut renderer,
                crate::components::app::AppAction::ClearScreen,
            )
            .await
            .unwrap();
        assert!(!app.state.exit_requested);
    }

    #[tokio::test]
    async fn submit_prompt_with_attachments_pushes_warning_lines_to_scrollback() {
        let mut renderer = Renderer::new(Vec::new(), Theme::default());
        let prompt_handle = AcpPromptHandle::noop();
        let session_id = acp::SessionId::new("test");
        let attachment = crate::components::app::PromptAttachment {
            path: std::path::PathBuf::from("missing-file.txt"),
            display_name: "missing-file.txt".to_string(),
        };

        submit_prompt_with_attachments(
            &mut renderer,
            &prompt_handle,
            &session_id,
            "hello",
            vec![attachment],
        )
        .await
        .unwrap();

        let output = String::from_utf8_lossy(renderer.writer());
        assert!(output.contains("[wisp]"));
        assert!(output.contains("missing-file.txt"));
    }
}
