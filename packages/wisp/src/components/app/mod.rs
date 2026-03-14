mod attachments;
pub mod controller;
pub mod git_diff_mode;
mod state;
pub mod view;

pub use controller::UiStateController;
pub use git_diff_mode::{GitDiffLoadState, GitDiffMode, GitDiffViewState, PatchFocus, ScreenMode};
pub use state::UiState;
use acp_utils::client::AcpEvent;
use crate::tui::{Line, Theme};
use std::path::PathBuf;

pub enum ViewEffect {
    ClearScreen,
    PushToScrollback(Vec<Line>),
    SetTheme(Theme),
}



/// Unified event type for the Wisp application.
pub enum WispEvent {
    Terminal(crate::tui::Event),
    Acp(AcpEvent),
}

#[derive(Debug, Clone)]
pub struct PromptAttachment {
    pub path: PathBuf,
    pub display_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::command_picker::CommandEntry;
    use crate::components::config_menu::ConfigMenu;
    use crate::settings::{ThemeSettings as WispThemeSettings, WispSettings, save_settings};
    use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
    use crate::tui::advanced::Renderer;
    use crate::tui::testing::render_component;
    use crate::tui::{Component, Frame, Theme, ViewContext};
    use acp_utils::client::AcpPromptHandle;
    use acp_utils::config_option_id::THEME_CONFIG_ID;
    use agent_client_protocol::{self as acp, SessionConfigOption};
    use std::fs;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    fn make_renderer() -> Renderer<Vec<u8>> {
        Renderer::new(Vec::new(), Theme::default())
    }

    fn render_view(
        renderer: &mut Renderer<Vec<u8>>,
        state: &mut UiState,
        context: &ViewContext,
    ) -> Frame {
        let ctx = renderer.context();
        state.prepare_for_render(&ctx);
        renderer
            .render_frame(|ctx| {
                view::build_frame(state, &state.git_diff_mode, &state.cached_visible_plan_entries, ctx)
            })
            .unwrap();
        view::build_frame(state, &state.git_diff_mode, &state.cached_visible_plan_entries, context)
    }

    #[allow(dead_code)]
    fn custom_theme() -> crate::tui::Theme {
        let temp_dir = TempDir::new().expect("temp dir");
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).expect("create themes dir");
        fs::write(themes_dir.join("custom.tmTheme"), CUSTOM_TMTHEME).expect("write theme file");

        let settings = WispSettings {
            theme: WispThemeSettings {
                file: Some("custom.tmTheme".to_string()),
            },
        };

        let mut theme = crate::tui::Theme::default();
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
            let state = UiState::new("test-agent".to_string(), &[], vec![], PathBuf::from("."));
            let menu = state.decorate_config_menu(ConfigMenu::from_config_options(&[]));

            assert_eq!(menu.options()[0].config_id, THEME_CONFIG_ID);
            assert_eq!(menu.options()[0].title, "Theme");
            assert_eq!(menu.options()[0].values[0].name, "Default");
            assert!(
                menu.options()[0]
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

            let state = UiState::new("test-agent".to_string(), &[], vec![], PathBuf::from("."));
            let menu = state.decorate_config_menu(ConfigMenu::from_config_options(&[]));
            let theme = &menu.options()[0];
            assert_eq!(theme.config_id, THEME_CONFIG_ID);
            assert_eq!(theme.current_raw_value, "nord.tmTheme");
            assert_eq!(
                theme.values[theme.current_value_index].value,
                "nord.tmTheme"
            );
        });
    }

    #[test]
    fn command_picker_cursor_stays_in_input_prompt() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![], PathBuf::from("."));
        let mut renderer = make_renderer();
        state
            .prompt_composer
            .open_command_picker_with_entries(vec![CommandEntry {
                name: "config".to_string(),
                description: "Open config".to_string(),
                has_input: false,
                hint: None,
                builtin: true,
            }]);

        let context = ViewContext::new((120, 40));
        let output = render_view(&mut renderer, &mut state, &context);
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
        let mut state = UiState::new("test-agent".to_string(), &options, vec![], PathBuf::from("."));
        let mut renderer = make_renderer();
        state.open_config_overlay();

        let context = ViewContext::new((120, 40));
        let output = render_view(&mut renderer, &mut state, &context);
        assert!(
            output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );

        state.config_overlay = None;
        let output = render_view(&mut renderer, &mut state, &context);
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
        let mut state = UiState::new("test-agent".to_string(), &[], vec![], PathBuf::from("."));
        let mut renderer = make_renderer();
        state.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "1",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Completed,
            )],
            Instant::now()
                .checked_sub(state.plan_tracker.grace_period + Duration::from_millis(1))
                .unwrap(),
        );
        state.plan_tracker.on_tick(Instant::now());

        let output = render_view(&mut renderer, &mut state, &ViewContext::new((120, 40)));
        assert!(
            !output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Plan"))
        );
    }

    #[test]
    fn plan_version_increments_on_replace() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![], PathBuf::from("."));

        let initial_version = state.plan_tracker.version();
        state.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "Task A",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Pending,
            )],
            Instant::now(),
        );

        assert!(state.plan_tracker.version() > initial_version);
    }

    #[test]
    fn plan_version_increments_on_clear() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![], PathBuf::from("."));

        state.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "Task A",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Pending,
            )],
            Instant::now(),
        );
        let version_before_clear = state.plan_tracker.version();
        state.plan_tracker.clear();

        assert!(state.plan_tracker.version() > version_before_clear);
    }

    #[tokio::test]
    async fn sessions_listed_filters_out_current_session() {
        let current_session_id = acp::SessionId::new("current-session");
        let mut controller = UiStateController::new(
            current_session_id.clone(),
            AcpPromptHandle::noop(),
        );
        let mut state = UiState::new("test-agent".to_string(), &[], vec![], PathBuf::from("."));

        let sessions = vec![
            acp::SessionInfo::new("other-session-1", PathBuf::from("/project"))
                .title("First other session".to_string()),
            acp::SessionInfo::new("current-session", PathBuf::from("/project"))
                .title("Current session title".to_string()),
            acp::SessionInfo::new("other-session-2", PathBuf::from("/other"))
                .title("Second other session".to_string()),
        ];

        controller
            .handle_event(
                &mut state,
                &ViewContext::new((60, 10)),
                WispEvent::Acp(AcpEvent::SessionsListed { sessions }),
            )
            .await
            .unwrap();

        let picker = state.session_picker.as_ref().unwrap();
        let term = render_component(|ctx| picker.render(ctx), 60, 10);
        let lines = term.get_lines();

        assert!(
            !lines
                .iter()
                .any(|line| line.contains("Current session title")),
            "current session should be filtered out, got: {:?}",
            lines
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("First other session")),
            "first other session should be present"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Second other session")),
            "second other session should be present"
        );
    }
}
