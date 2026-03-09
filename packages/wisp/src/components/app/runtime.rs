use super::state::{GitDiffLoadState, GitDiffViewState, ScreenMode};
use super::{App, AppAction, PromptAttachment, build_attachment_blocks};
use crate::settings::{load_or_create_settings, save_settings};
use crate::tui;
use crate::tui::{Action, Line, Renderer};
use std::io::Write;

impl App {
    #[allow(clippy::too_many_lines)]
    pub async fn apply_effect<T: Write>(
        &mut self,
        renderer: &mut Renderer<T>,
        effect: AppAction,
    ) -> Result<Vec<Action<AppAction>>, Box<dyn std::error::Error>> {
        match effect {
            AppAction::PushScrollback(lines) => {
                renderer.push_to_scrollback(&lines)?;
                Ok(vec![])
            }
            AppAction::PromptSubmit {
                user_input,
                attachments,
            } => {
                submit_prompt_with_attachments(
                    renderer,
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
                apply_theme_selection(renderer, file);
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
                renderer.clear_screen()?;
                Ok(vec![])
            }
            AppAction::ToggleGitDiffViewer => {
                self.state.screen_mode =
                    ScreenMode::GitDiff(GitDiffViewState::new(GitDiffLoadState::Loading));
                self.state.bump_render_version();
                tui::set_mouse_capture(true);
                self.load_git_diff().await;
                Ok(vec![])
            }
            AppAction::RefreshGitDiffViewer => {
                if let ScreenMode::GitDiff(ref mut diff_state) = self.state.screen_mode {
                    let prev_path = match &diff_state.load_state {
                        GitDiffLoadState::Ready(doc) => doc
                            .files
                            .get(diff_state.selected_file)
                            .map(|f| f.path.clone()),
                        _ => None,
                    };
                    let focus = diff_state.focus;
                    diff_state.load_state = GitDiffLoadState::Loading;
                    self.state.bump_render_version();
                    self.load_git_diff().await;

                    if let ScreenMode::GitDiff(ref mut diff_state) = self.state.screen_mode {
                        diff_state.focus = focus;
                        diff_state.patch_scroll = 0;
                        if let (Some(prev), GitDiffLoadState::Ready(doc)) =
                            (&prev_path, &diff_state.load_state)
                        {
                            diff_state.selected_file = doc
                                .files
                                .iter()
                                .position(|f| f.path == *prev)
                                .unwrap_or(0);
                        }
                    }
                }
                Ok(vec![])
            }
        }
    }

    async fn load_git_diff(&mut self) {
        match crate::git_diff::load_git_diff(&self.working_dir, self.cached_repo_root.as_deref())
            .await
        {
            Ok(doc) => {
                if self.cached_repo_root.is_none() {
                    self.cached_repo_root = Some(doc.repo_root.clone());
                }
                if doc.files.is_empty() {
                    if let ScreenMode::GitDiff(ref mut diff_state) = self.state.screen_mode {
                        diff_state.load_state = GitDiffLoadState::Empty;
                        diff_state.invalidate_patch_cache();
                    }
                } else if let ScreenMode::GitDiff(ref mut diff_state) = self.state.screen_mode {
                    let file_count = doc.files.len();
                    diff_state.load_state = GitDiffLoadState::Ready(doc);
                    diff_state.selected_file =
                        diff_state.selected_file.min(file_count.saturating_sub(1));
                    diff_state.invalidate_patch_cache();
                }
            }
            Err(e) => {
                if let ScreenMode::GitDiff(ref mut diff_state) = self.state.screen_mode {
                    diff_state.load_state = GitDiffLoadState::Error {
                        message: e.to_string(),
                    };
                    diff_state.invalidate_patch_cache();
                }
            }
        }
        self.state.bump_render_version();
    }
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
        renderer.push_to_scrollback(&warning_lines)?;
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
    use crate::tui::Color;
    use crate::tui::Renderer;
    use crate::tui::theme::Theme;
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

        assert_eq!(renderer.render_epoch(), 1);
    }
}
