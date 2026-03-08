mod attachments;
mod session;
mod state;

pub use state::AppAction;
pub(crate) use state::UiState;

use crate::components::container::Container;
use crate::components::conversation_window::ConversationWindow;
use crate::components::plan_view::PlanView;
use crate::components::status_line::StatusLine;
use crate::tui::{Component, Cursor, CursorComponent, Line, RenderContext, RenderOutput};
use acp_utils::notifications::McpServerStatus;
use agent_client_protocol::{self as acp, SessionConfigOption};
use std::path::PathBuf;
use std::time::Instant;

pub use attachments::build_attachment_blocks;

/// Runtime-executed side effects emitted by the app state machine.
#[derive(Debug)]
pub enum AppEffect {
    Exit,
    Render,
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
}

#[derive(Debug, Clone)]
pub struct PromptAttachment {
    pub path: PathBuf,
    pub display_name: String,
}

pub struct App {
    state: UiState,
}

impl App {
    pub fn new(
        agent_name: String,
        config_options: &[SessionConfigOption],
        auth_methods: Vec<acp::AuthMethod>,
    ) -> Self {
        Self {
            state: UiState::new(agent_name, config_options, auth_methods),
        }
    }

    pub fn dispatch(&mut self, action: AppAction, context: &RenderContext) -> Vec<AppEffect> {
        match action {
            AppAction::Key(key_event) => self.state.on_key_event(key_event),
            AppAction::Paste(text) => self.state.on_paste(text),
            AppAction::Resize { cols, rows } => self.state.on_resize(cols, rows),
            AppAction::Tick => self.state.on_tick(),
            AppAction::SessionUpdate(update) => self.state.on_session_update(update),
            AppAction::ExtNotification(notification) => {
                self.state.on_ext_notification(notification)
            }
            AppAction::PromptDone => self.state.on_prompt_done(context),
            AppAction::PromptError => self.state.on_prompt_error(),
            AppAction::ElicitationRequest {
                params,
                response_tx,
            } => self.state.on_elicitation_request(params, response_tx),
            AppAction::AuthenticateComplete { method_id } => {
                self.state.on_authenticate_complete(&method_id);
                vec![AppEffect::Render]
            }
            AppAction::AuthenticateFailed { method_id, error } => {
                tracing::warn!("Provider auth failed for {method_id}: {error}");
                self.state.on_authenticate_failed(&method_id);
                vec![AppEffect::Render]
            }
            AppAction::SetFilePickerMatches(matches) => {
                self.state
                    .prompt_composer
                    .open_file_picker_with_matches(matches);
                vec![AppEffect::Render]
            }
        }
    }

    pub(crate) fn on_authenticate_started(&mut self, method_id: &str) {
        self.state.on_authenticate_started(method_id);
    }
}

impl CursorComponent for App {
    fn render(&mut self, context: &RenderContext) -> RenderOutput {
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
                logical_row: overlay.cursor_row_offset(),
                col: overlay.cursor_col(),
            };

            let container = Container::new(vec![overlay as &dyn Component, &status_line]);
            let (lines, _) = container.render_with_offsets(context);

            return RenderOutput {
                lines,
                cursor,
                cursor_visible: overlay.has_picker(),
            };
        }

        let grace_period = self.state.plan_tracker.grace_period;
        let visible_plan_entries = self
            .state
            .plan_tracker
            .visible_entries(Instant::now(), grace_period);

        let progress = self.state.tool_call_statuses.progress();
        self.state
            .progress_indicator
            .update(progress.completed_top_level, progress.total_top_level);

        self.state
            .conversation
            .ensure_all_rendered(&self.state.tool_call_statuses, context);

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
            logical_row: offsets[prompt_component_index] + prompt_cursor.logical_row,
            col: prompt_cursor.col,
        };

        RenderOutput {
            lines,
            cursor,
            cursor_visible: true,
        }
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
    use crate::components::config_menu::ConfigMenu;
    use crate::components::config_overlay::ConfigOverlayAction;
    use crate::settings::{ThemeSettings as WispThemeSettings, WispSettings, save_settings};
    use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
    use crate::tui::KeyEventResponse;
    use acp_utils::config_option_id::THEME_CONFIG_ID;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
            let app = App::new("test-agent".to_string(), &[], vec![]);
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

            let app = App::new("test-agent".to_string(), &[], vec![]);
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
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let outcome =
            KeyEventResponse::action_and_render(ConfigOverlayAction::ApplyConfigChanges(vec![
                crate::components::config_menu::ConfigChange {
                    config_id: THEME_CONFIG_ID.to_string(),
                    new_value: "catppuccin.tmTheme".to_string(),
                },
            ]));

        let effects = app.state.handle_config_overlay_outcome(outcome);

        assert!(matches!(
            effects.as_slice(),
            [
                AppEffect::SetTheme {
                    file: Some(file)
                },
                AppEffect::Render
            ] if file == "catppuccin.tmTheme"
        ));
    }

    #[test]
    fn theme_default_value_maps_to_none() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let outcome =
            KeyEventResponse::action_and_render(ConfigOverlayAction::ApplyConfigChanges(vec![
                crate::components::config_menu::ConfigChange {
                    config_id: THEME_CONFIG_ID.to_string(),
                    new_value: "   ".to_string(),
                },
            ]));

        let effects = app.state.handle_config_overlay_outcome(outcome);

        assert!(matches!(
            effects.as_slice(),
            [AppEffect::SetTheme { file: None }, AppEffect::Render]
        ));
    }

    #[test]
    fn non_theme_config_change_still_emits_set_config_option() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let outcome =
            KeyEventResponse::action_and_render(ConfigOverlayAction::ApplyConfigChanges(vec![
                crate::components::config_menu::ConfigChange {
                    config_id: "model".to_string(),
                    new_value: "gpt-5".to_string(),
                },
            ]));

        let effects = app.state.handle_config_overlay_outcome(outcome);

        assert!(matches!(
            effects.as_slice(),
            [
                AppEffect::SetConfigOption {
                    config_id,
                    new_value
                },
                AppEffect::Render
            ] if config_id == "model" && new_value == "gpt-5"
        ));
    }

    #[test]
    fn command_picker_cursor_stays_in_input_prompt() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
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
            .lines
            .iter()
            .position(|line| line.plain_text().contains("> "))
            .expect("input prompt should exist");
        assert_eq!(output.cursor.logical_row, input_row);
    }

    #[test]
    fn config_overlay_replaces_conversation_window() {
        let options = vec![acp::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![acp::SessionConfigSelectOption::new("m1", "M1")],
        )];
        let mut screen = App::new("test-agent".to_string(), &options, vec![]);
        screen.state.open_config_overlay();

        let context = RenderContext::new((120, 40));
        let output = screen.render(&context);
        assert!(
            output
                .lines
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );

        screen.state.config_overlay = None;
        let output = screen.render(&context);
        assert!(
            !output
                .lines
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );
    }

    #[test]
    fn shift_tab_cycles_mode_option() {
        use agent_client_protocol::SessionConfigOptionCategory;

        let options = vec![
            SessionConfigOption::select(
                "mode",
                "Mode",
                "Planner",
                vec![
                    acp::SessionConfigSelectOption::new("Planner", "Planner"),
                    acp::SessionConfigSelectOption::new("Coder", "Coder"),
                ],
            )
            .category(SessionConfigOptionCategory::Mode),
        ];

        let mut app = App::new("test-agent".to_string(), &options, vec![]);
        let effects = app.dispatch(
            AppAction::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            &RenderContext::new((120, 40)),
        );

        assert!(effects.iter().any(|event| {
            matches!(
                event,
                AppEffect::SetConfigOption { config_id, new_value }
                if config_id == "mode" && new_value == "Coder"
            )
        }));
    }

    #[test]
    fn shift_tab_wraps_mode_option() {
        use agent_client_protocol::SessionConfigOptionCategory;

        let options = vec![
            SessionConfigOption::select(
                "mode",
                "Mode",
                "Coder",
                vec![
                    acp::SessionConfigSelectOption::new("Planner", "Planner"),
                    acp::SessionConfigSelectOption::new("Coder", "Coder"),
                ],
            )
            .category(SessionConfigOptionCategory::Mode),
        ];

        let mut app = App::new("test-agent".to_string(), &options, vec![]);
        let effects = app.dispatch(
            AppAction::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            &RenderContext::new((120, 40)),
        );

        assert!(effects.iter().any(|event| {
            matches!(
                event,
                AppEffect::SetConfigOption { config_id, new_value }
                if config_id == "mode" && new_value == "Planner"
            )
        }));
    }

    #[test]
    fn shift_tab_ignored_when_overlay_consumes_input() {
        use agent_client_protocol::SessionConfigOptionCategory;

        let options = vec![
            SessionConfigOption::select(
                "mode",
                "Mode",
                "Planner",
                vec![acp::SessionConfigSelectOption::new("Planner", "Planner")],
            )
            .category(SessionConfigOptionCategory::Mode),
        ];

        let mut app = App::new("test-agent".to_string(), &options, vec![]);
        app.state.open_config_overlay();

        let effects = app.dispatch(
            AppAction::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            &RenderContext::new((120, 40)),
        );
        assert!(
            !effects
                .iter()
                .any(|event| matches!(event, AppEffect::SetConfigOption { .. }))
        );
    }

    #[test]
    fn shift_tab_noop_when_no_cycleable_option_exists() {
        use agent_client_protocol::SessionConfigOptionCategory;

        let options = vec![
            SessionConfigOption::select(
                "model",
                "Model",
                "m1",
                vec![
                    acp::SessionConfigSelectOption::new("m1", "M1"),
                    acp::SessionConfigSelectOption::new("m2", "M2"),
                ],
            )
            .category(SessionConfigOptionCategory::Model),
        ];

        let mut app = App::new("test-agent".to_string(), &options, vec![]);
        let effects = app.dispatch(
            AppAction::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            &RenderContext::new((120, 40)),
        );
        assert!(effects.is_empty());
    }

    #[test]
    fn ctrl_c_emits_exit() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        let effects = screen.dispatch(
            AppAction::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            &RenderContext::new((120, 40)),
        );
        assert!(matches!(effects.as_slice(), [AppEffect::Exit]));
    }

    #[test]
    fn escape_while_waiting_emits_cancel() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen.state.waiting_for_response = true;

        let effects = screen.dispatch(
            AppAction::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            &RenderContext::new((120, 40)),
        );
        assert!(matches!(effects.as_slice(), [AppEffect::Cancel]));
    }

    #[test]
    fn escape_while_not_waiting_does_nothing() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen.state.waiting_for_response = false;

        let effects = screen.dispatch(
            AppAction::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            &RenderContext::new((120, 40)),
        );
        assert!(effects.is_empty());
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
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
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
                .lines
                .iter()
                .any(|line| line.plain_text().contains("Plan"))
        );
    }
}
