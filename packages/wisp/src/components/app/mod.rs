mod attachments;
pub(crate) mod git_diff_mode;
pub mod runtime;
mod session;
mod state;

pub(crate) use git_diff_mode::{
    GitDiffLoadState, GitDiffMode, GitDiffViewState, PatchFocus, QueuedComment, ScreenMode,
};
pub(crate) use state::UiState;

use crate::tui::{
    Action, App as TuiApp, Component, Cursor, Frame, Line, RenderContext, Renderer, RootComponent,
    TerminalEvent,
};
use acp_utils::client::{AcpEvent, AcpPromptHandle};
use acp_utils::notifications::McpServerStatus;
use agent_client_protocol::{self as acp, SessionConfigOption};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use crate::components::container::Container;
use crate::components::conversation_window::ConversationWindow;
use crate::components::plan_view::PlanView;
use crate::components::status_line::StatusLine;

pub use attachments::build_attachment_blocks;

/// Runtime-executed side effects emitted by the app state machine.
#[derive(Debug)]
#[allow(private_interfaces)]
pub enum AppAction {
    PushScrollback(Vec<Line>),
    PromptSubmit {
        user_input: String,
        attachments: Vec<PromptAttachment>,
    },
    SetConfigOption {
        config_id: String,
        new_value: String,
    },
    SetTheme {
        file: Option<String>,
    },
    Cancel,
    AuthenticateMcpServer {
        server_name: String,
    },
    AuthenticateProvider {
        method_id: String,
    },
    ClearScreen,
    OpenGitDiffViewer,
    RefreshGitDiffViewer,
    CloseGitDiffViewer,
    SubmitDiffReview {
        comments: Vec<QueuedComment>,
    },
}

#[derive(Debug, Clone)]
pub struct PromptAttachment {
    pub path: PathBuf,
    pub display_name: String,
}

pub struct App {
    state: UiState,
    prompt_handle: AcpPromptHandle,
    session_id: acp::SessionId,
    git_diff_mode: GitDiffMode,
}

impl App {
    pub fn new(
        agent_name: String,
        config_options: &[SessionConfigOption],
        auth_methods: Vec<acp::AuthMethod>,
        prompt_handle: AcpPromptHandle,
        session_id: acp::SessionId,
        working_dir: PathBuf,
    ) -> Self {
        Self {
            state: UiState::new(agent_name, config_options, auth_methods),
            prompt_handle,
            session_id,
            git_diff_mode: GitDiffMode::new(working_dir),
        }
    }
}

impl TuiApp for App {
    type Event = AcpEvent;
    type Action = AppAction;
    type Error = Box<dyn std::error::Error>;

    fn on_terminal_event(
        &mut self,
        event: TerminalEvent,
        _context: &RenderContext,
    ) -> Vec<Action<AppAction>> {
        match event {
            TerminalEvent::Key(key_event) => {
                let input = self.state.on_key_event(key_event);
                if matches!(self.state.screen_mode, ScreenMode::GitDiff) {
                    let interaction = self.git_diff_mode.on_key_event(key_event);
                    let mut actions = input.actions;
                    actions.extend(interaction.actions);
                    if interaction.changed {
                        self.state.bump_render_version();
                    }
                    actions
                } else {
                    input.actions
                }
            }
            TerminalEvent::Paste(text) => self.state.on_paste(text),
            TerminalEvent::Mouse(mouse) => self
                .state
                .on_mouse_event(mouse, Some(&mut self.git_diff_mode)),
        }
    }

    fn on_tick(&mut self, _context: &RenderContext) -> Vec<Action<AppAction>> {
        self.state.on_tick()
    }

    fn render_version(&self) -> u64 {
        self.state.render_version()
    }

    fn wants_tick(&self) -> bool {
        self.state.wants_tick()
    }

    fn on_event(&mut self, event: AcpEvent, context: &RenderContext) -> Vec<Action<AppAction>> {
        match event {
            AcpEvent::SessionUpdate(update) => self.state.on_session_update(*update),
            AcpEvent::ExtNotification(notification) => self.state.on_ext_notification(notification),
            AcpEvent::PromptDone(_) => self.state.on_prompt_done(context),
            AcpEvent::PromptError(error) => self.state.on_prompt_error(&error),
            AcpEvent::ElicitationRequest {
                params,
                response_tx,
            } => self.state.on_elicitation_request(params, response_tx),
            AcpEvent::AuthenticateComplete { method_id } => {
                self.state.on_authenticate_complete(&method_id)
            }
            AcpEvent::AuthenticateFailed { method_id, error } => {
                self.state.on_authenticate_failed(&method_id, &error)
            }
            AcpEvent::ConnectionClosed => self.state.on_connection_closed(),
        }
    }

    async fn on_action<T: Write>(
        &mut self,
        renderer: &mut Renderer<T>,
        effect: Self::Action,
    ) -> Result<Vec<Action<Self::Action>>, Self::Error> {
        self.apply_effect(renderer, effect).await
    }
}

impl RootComponent for App {
    fn prepare_render(&mut self, context: &RenderContext) {
        self.state
            .prepare_render(context, Some(&mut self.git_diff_mode));
    }

    fn render(&mut self, context: &RenderContext) -> Frame {
        let unhealthy_count = self
            .state
            .server_statuses
            .iter()
            .filter(|status| !matches!(status.status, McpServerStatus::Connected { .. }))
            .count();
        let status_line = StatusLine {
            agent_name: &self.state.agent_name,
            mode_display: self.state.mode_display.as_deref(),
            model_display: self.state.model_display.as_deref(),
            reasoning_effort: self.state.reasoning_effort,
            context_pct_left: self.state.context_usage_pct,
            waiting_for_response: self.state.waiting_for_response,
            unhealthy_server_count: unhealthy_count,
        };

        if let Some(ref mut overlay) = self.state.config_overlay {
            overlay.prepare_render(context);
            let cursor = Cursor {
                row: overlay.cursor_row_offset(),
                col: overlay.cursor_col(),
                is_visible: overlay.has_picker(),
            };

            let container = Container::new(vec![overlay as &dyn Component, &status_line]);
            let (lines, _) = container.render_with_offsets(context);

            return Frame::new(lines, cursor);
        }

        if matches!(self.state.screen_mode, ScreenMode::GitDiff) {
            let status_lines = status_line.render(context);
            #[allow(clippy::cast_possible_truncation)]
            let diff_height = context
                .size
                .height
                .saturating_sub(status_lines.len() as u16);
            let diff_context = context.with_size((context.size.width, diff_height));
            let line_count = diff_height as usize;
            let mut lines = self.git_diff_mode.render(&diff_context);
            lines.extend(status_lines);

            let cursor = if self.git_diff_mode.is_comment_input() {
                let comment_cursor = self.git_diff_mode.comment_cursor_col();
                Cursor {
                    row: line_count.saturating_sub(1),
                    col: "Comment: ".len() + comment_cursor,
                    is_visible: true,
                }
            } else {
                Cursor {
                    row: 0,
                    col: 0,
                    is_visible: false,
                }
            };

            return Frame::new(lines, cursor);
        }

        let grace_period = self.state.plan_tracker.grace_period;
        let visible_plan_entries = self
            .state
            .plan_tracker
            .visible_entries(Instant::now(), grace_period);

        let conversation_window = ConversationWindow {
            loader: &self.state.grid_loader,
            conversation: &self.state.conversation,
        };
        let plan_view = PlanView {
            entries: &visible_plan_entries,
        };

        let mut container: Container<'_> = Container::new(vec![
            &conversation_window,
            &plan_view,
            &self.state.progress_indicator,
            &self.state.prompt_composer,
        ]);
        let prompt_component_index = container.len() - 1;

        if let Some(ref elicitation_form) = self.state.elicitation_form {
            container.push(&elicitation_form.form);
        }

        container.push(&status_line);
        let (lines, offsets) = container.render_with_offsets(context);
        let prompt_cursor = self.state.prompt_composer.cursor(context);
        let cursor = Cursor {
            row: offsets[prompt_component_index] + prompt_cursor.row,
            col: prompt_cursor.col,
            is_visible: true,
        };

        Frame::new(lines, cursor)
    }
}

fn theme_file_from_picker_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::command_picker::CommandEntry;
    use crate::components::config_menu::{ConfigChange, ConfigMenu};
    use crate::components::config_overlay::ConfigOverlayMessage;
    use crate::settings::{ThemeSettings as WispThemeSettings, WispSettings, save_settings};
    use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
    use crate::tui::MessageResult;
    use acp_utils::config_option_id::THEME_CONFIG_ID;
    use std::fs;
    use std::time::Duration;
    use tempfile::TempDir;

    #[allow(dead_code)]
    fn custom_theme() -> crate::tui::theme::Theme {
        let temp_dir = TempDir::new().expect("temp dir");
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).expect("create themes dir");
        fs::write(themes_dir.join("custom.tmTheme"), CUSTOM_TMTHEME).expect("write theme file");

        let settings = WispSettings {
            theme: WispThemeSettings {
                file: Some("custom.tmTheme".to_string()),
            },
        };

        let mut theme = crate::tui::theme::Theme::default();
        with_wisp_home(temp_dir.path(), || {
            theme = crate::settings::load_theme(&settings);
        });
        theme
    }

    #[test]
    fn decorate_config_menu_adds_theme_entry() {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("catppuccin.tmTheme"), "x").unwrap();

        with_wisp_home(temp_dir.path(), || {
            let app = App::new(
                "test-agent".to_string(),
                &[],
                vec![],
                AcpPromptHandle::noop(),
                acp::SessionId::new("test"),
                PathBuf::from("."),
            );
            let menu = app
                .state
                .decorate_config_menu(ConfigMenu::from_config_options(&[]));

            assert_eq!(menu.options[0].config_id, THEME_CONFIG_ID);
            assert_eq!(menu.options[0].title, "Theme");
            assert_eq!(menu.options[0].values[0].name, "Default");
            assert!(
                menu.options[0]
                    .values
                    .iter()
                    .any(|value| value.value == "catppuccin.tmTheme")
            );
        });
    }

    #[test]
    fn theme_entry_uses_current_theme_from_settings() {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("catppuccin.tmTheme"), "x").unwrap();
        fs::write(themes_dir.join("nord.tmTheme"), "x").unwrap();

        with_wisp_home(temp_dir.path(), || {
            let settings = WispSettings {
                theme: WispThemeSettings {
                    file: Some("nord.tmTheme".to_string()),
                },
            };
            save_settings(&settings).unwrap();

            let app = App::new(
                "test-agent".to_string(),
                &[],
                vec![],
                AcpPromptHandle::noop(),
                acp::SessionId::new("test"),
                PathBuf::from("."),
            );
            let menu = app
                .state
                .decorate_config_menu(ConfigMenu::from_config_options(&[]));
            let theme = &menu.options[0];
            assert_eq!(theme.config_id, THEME_CONFIG_ID);
            assert_eq!(theme.current_raw_value, "nord.tmTheme");
            assert_eq!(
                theme.values[theme.current_value_index].value,
                "nord.tmTheme"
            );
        });
    }

    #[test]
    fn theme_config_change_emits_set_theme_event() {
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );
        let outcome = MessageResult::message(ConfigOverlayMessage::ApplyConfigChanges(vec![
            ConfigChange {
                config_id: THEME_CONFIG_ID.to_string(),
                new_value: "catppuccin.tmTheme".to_string(),
            },
        ]));

        let effects = app.state.handle_config_overlay_messages(outcome);

        assert!(matches!(
            effects.as_slice(),
            [Action::Custom(AppAction::SetTheme {
                file: Some(file)
            })] if file == "catppuccin.tmTheme"
        ));
    }

    #[test]
    fn theme_default_value_maps_to_none() {
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );
        let outcome = MessageResult::message(ConfigOverlayMessage::ApplyConfigChanges(vec![
            ConfigChange {
                config_id: THEME_CONFIG_ID.to_string(),
                new_value: "   ".to_string(),
            },
        ]));

        let effects = app.state.handle_config_overlay_messages(outcome);

        assert!(matches!(
            effects.as_slice(),
            [Action::Custom(AppAction::SetTheme { file: None })]
        ));
    }

    #[test]
    fn non_theme_config_change_still_emits_set_config_option() {
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );
        let outcome = MessageResult::message(ConfigOverlayMessage::ApplyConfigChanges(vec![
            ConfigChange {
                config_id: "model".to_string(),
                new_value: "gpt-5".to_string(),
            },
        ]));

        let effects = app.state.handle_config_overlay_messages(outcome);

        assert!(matches!(
            effects.as_slice(),
            [Action::Custom(AppAction::SetConfigOption {
                config_id,
                new_value
            })] if config_id == "model" && new_value == "gpt-5"
        ));
    }

    #[test]
    fn command_picker_cursor_stays_in_input_prompt() {
        let mut screen = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );
        screen
            .state
            .prompt_composer
            .open_command_picker_with_entries(vec![CommandEntry {
                name: "config".to_string(),
                description: "Open config".to_string(),
                has_input: false,
                hint: None,
                builtin: true,
            }]);

        let context = RenderContext::new((120, 40));
        let output = screen.render(&context);
        let input_row = output
            .lines()
            .iter()
            .position(|line| line.plain_text().contains("> "))
            .expect("input prompt should exist");
        assert_eq!(output.cursor().row, input_row);
    }

    #[test]
    fn config_overlay_replaces_conversation_window() {
        let options = vec![acp::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![acp::SessionConfigSelectOption::new("m1", "M1")],
        )];
        let mut screen = App::new(
            "test-agent".to_string(),
            &options,
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );
        screen.state.open_config_overlay();

        let context = RenderContext::new((120, 40));
        let output = screen.render(&context);
        assert!(
            output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );

        screen.state.config_overlay = None;
        let output = screen.render(&context);
        assert!(
            !output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );
    }

    #[test]
    fn extract_model_display_handles_comma_separated_value() {
        use state::extract_model_display;

        let options = vec![SessionConfigOption::select(
            "model",
            "Model",
            "a:x,b:y",
            vec![
                acp::SessionConfigSelectOption::new("a:x", "Alpha / X"),
                acp::SessionConfigSelectOption::new("b:y", "Beta / Y"),
                acp::SessionConfigSelectOption::new("c:z", "Gamma / Z"),
            ],
        )];
        assert_eq!(
            extract_model_display(&options).as_deref(),
            Some("Alpha / X + Beta / Y")
        );
    }

    #[test]
    fn extract_reasoning_effort_returns_none_for_none_value() {
        use acp_utils::config_option_id::ConfigOptionId;
        use state::extract_reasoning_effort;

        let options = vec![SessionConfigOption::select(
            ConfigOptionId::ReasoningEffort.as_str(),
            "Reasoning",
            "none",
            vec![
                acp::SessionConfigSelectOption::new("none", "None"),
                acp::SessionConfigSelectOption::new("low", "Low"),
            ],
        )];
        assert_eq!(extract_reasoning_effort(&options), None);
    }

    #[test]
    fn render_hides_plan_header_when_no_entries_are_visible() {
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );
        app.state.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "1",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Completed,
            )],
            Instant::now()
                .checked_sub(app.state.plan_tracker.grace_period + Duration::from_millis(1))
                .unwrap(),
        );

        let output = app.render(&RenderContext::new((120, 40)));
        assert!(
            !output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Plan"))
        );
    }
}
