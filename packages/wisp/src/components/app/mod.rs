mod attachments;
pub(crate) mod git_diff_mode;
pub mod runtime;
mod session;
mod state;

pub(crate) use git_diff_mode::{
    GitDiffLoadState, GitDiffMode, GitDiffViewState, PatchFocus, QueuedComment, ScreenMode,
};
pub(crate) use state::UiState;

use crate::tui::advanced::Renderer;
use crate::tui::{
    App as TuiApp, AppEvent, Component, Cursor, Event, Frame, Layout, Line, ViewContext, merge,
};
use acp_utils::client::{AcpEvent, AcpPromptHandle};
use acp_utils::notifications::McpServerStatus;
use agent_client_protocol::{self as acp, SessionConfigOption};
use std::path::PathBuf;
use std::time::Instant;
use utils::ReasoningEffort;

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
    ListSessions,
    LoadSession {
        session_id: String,
        cwd: PathBuf,
    },
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

struct StatusLineProps {
    agent_name: String,
    mode_display: Option<String>,
    model_display: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    context_pct_left: Option<u8>,
    waiting_for_response: bool,
    unhealthy_server_count: usize,
}

pub struct App {
    state: UiState,
    prompt_handle: AcpPromptHandle,
    session_id: acp::SessionId,
    git_diff_mode: GitDiffMode,
    cached_visible_plan_entries: Vec<acp::PlanEntry>,
    cached_plan_version: u64,
    cached_plan_tick: Instant,
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
            cached_visible_plan_entries: Vec::new(),
            cached_plan_version: 0,
            cached_plan_tick: Instant::now(),
        }
    }

    fn prepare_for_view(&mut self, context: &ViewContext) {
        self.state
            .refresh_caches(context, Some(&mut self.git_diff_mode));

        if let Some(ref mut overlay) = self.state.config_overlay {
            let height = (context.size.height.saturating_sub(1)) as usize;
            if height >= 3 {
                overlay.update_child_viewport(height.saturating_sub(4));
            }
        }

        let plan_version = self.state.plan_tracker.version();
        let last_tick = self.state.plan_tracker.last_tick();
        if plan_version != self.cached_plan_version || last_tick != self.cached_plan_tick {
            let grace_period = self.state.plan_tracker.grace_period;
            self.cached_visible_plan_entries = self
                .state
                .plan_tracker
                .visible_entries(last_tick, grace_period);
            self.cached_plan_version = plan_version;
            self.cached_plan_tick = last_tick;
        }
    }

    fn status_line_props(&self) -> StatusLineProps {
        let unhealthy_count = self
            .state
            .server_statuses
            .iter()
            .filter(|status| !matches!(status.status, McpServerStatus::Connected { .. }))
            .count();
        StatusLineProps {
            agent_name: self.state.agent_name.clone(),
            mode_display: self.state.mode_display.clone(),
            model_display: self.state.model_display.clone(),
            reasoning_effort: self.state.reasoning_effort,
            context_pct_left: self.state.context_usage_pct,
            waiting_for_response: self.state.waiting_for_response,
            unhealthy_server_count: unhealthy_count,
        }
    }
}

impl TuiApp for App {
    type Event = AcpEvent;
    type Effect = AppAction;
    type Error = Box<dyn std::error::Error>;

    fn update(
        &mut self,
        event: AppEvent<Self::Event>,
        context: &ViewContext,
    ) -> Option<Vec<Self::Effect>> {
        let effects = match event {
            AppEvent::Key(key_event) => {
                let event = Event::Key(key_event);
                let input = self.state.on_event(&event);
                if matches!(self.state.screen_mode, ScreenMode::GitDiff) {
                    let git_effects = self.git_diff_mode.on_event(&event);
                    merge(input, git_effects)
                } else {
                    input
                }
            }
            AppEvent::Paste(text) => {
                let event = Event::Paste(text);
                self.state.on_event(&event)
            }
            AppEvent::Mouse(mouse) => {
                if matches!(self.state.screen_mode, ScreenMode::GitDiff) {
                    self.git_diff_mode.on_event(&Event::Mouse(mouse));
                }
                Some(vec![])
            }
            AppEvent::Tick(_) => self.state.on_event(&Event::Tick),
            AppEvent::Resize(_) => Some(vec![]),
            AppEvent::External(event) => match event {
                AcpEvent::SessionUpdate(update) => self.state.on_session_update(*update),
                AcpEvent::ExtNotification(notification) => {
                    self.state.on_ext_notification(notification)
                }
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
                AcpEvent::SessionsListed { sessions } => {
                    self.state.open_session_picker(sessions);
                    Some(vec![])
                }
                AcpEvent::SessionLoaded {
                    session_id,
                    config_options,
                } => {
                    self.session_id = session_id;
                    self.state.update_config_options(&config_options);
                    Some(vec![])
                }
                AcpEvent::ConnectionClosed => self.state.on_connection_closed(),
            },
        };

        if !self.state.exit_requested {
            self.prepare_for_view(context);
        }

        effects
    }

    fn view(&self, context: &ViewContext) -> Frame {
        let s = self.status_line_props();
        let status_line = StatusLine {
            agent_name: &s.agent_name,
            mode_display: s.mode_display.as_deref(),
            model_display: s.model_display.as_deref(),
            reasoning_effort: s.reasoning_effort,
            context_pct_left: s.context_pct_left,
            waiting_for_response: s.waiting_for_response,
            unhealthy_server_count: s.unhealthy_server_count,
        };

        if let Some(ref overlay) = self.state.config_overlay {
            let cursor = Cursor {
                row: overlay.cursor_row_offset(),
                col: overlay.cursor_col(),
                is_visible: overlay.has_picker(),
            };

            let mut layout = Layout::new();
            layout.section(overlay.render(context));
            layout.section(status_line.render(context));
            let mut frame = layout.into_frame();
            // Override cursor from overlay (not section-relative)
            frame = Frame::new(frame.lines().to_vec(), cursor);
            return frame;
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

            let mut layout = Layout::new();
            layout.section(self.git_diff_mode.render(&diff_context));
            layout.section(status_lines);
            let frame = layout.into_frame();
            return Frame::new(frame.lines().to_vec(), cursor);
        }

        let conversation_window = ConversationWindow {
            loader: &self.state.grid_loader,
            conversation: &self.state.conversation,
        };
        let plan_view = PlanView {
            entries: &self.cached_visible_plan_entries,
        };

        let mut layout = Layout::new();
        layout.section(conversation_window.render(context));
        layout.section(plan_view.render(context));
        layout.section(self.state.progress_indicator.render(context));
        layout.section_with_cursor(
            self.state.prompt_composer.render(context),
            self.state.prompt_composer.cursor(context),
        );
        if let Some(ref session_picker) = self.state.session_picker {
            layout.section(session_picker.render(context));
        }
        if let Some(ref elicitation_form) = self.state.elicitation_form {
            layout.section(elicitation_form.form.render(context));
        }
        layout.section(status_line.render(context));
        layout.into_frame()
    }

    async fn run_effect(
        &mut self,
        terminal: &mut Renderer<impl std::io::Write>,
        effect: Self::Effect,
    ) -> Result<Vec<Self::Effect>, Self::Error> {
        let follow_up = self.apply_action(terminal, effect).await?;
        self.prepare_for_view(&terminal.context());
        Ok(follow_up)
    }

    fn should_exit(&self) -> bool {
        self.state.exit_requested
    }

    fn wants_tick(&self) -> bool {
        self.state.wants_tick()
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
    use crate::tui::Event;
    use acp_utils::config_option_id::THEME_CONFIG_ID;
    use std::fs;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    fn render_app(app: &mut App, context: &ViewContext) -> Frame {
        app.prepare_for_view(context);
        app.view(context)
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
    fn theme_config_change_emits_set_theme_event() {
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );
        let outcome = Some(vec![ConfigOverlayMessage::ApplyConfigChanges(vec![
            ConfigChange {
                config_id: THEME_CONFIG_ID.to_string(),
                new_value: "catppuccin.tmTheme".to_string(),
            },
        ])]);

        let effects = app.state.handle_config_overlay_messages(outcome);

        let actions = effects.unwrap_or_default();
        assert!(matches!(
            actions.as_slice(),
            [AppAction::SetTheme {
                file: Some(file)
            }] if file == "catppuccin.tmTheme"
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
        let outcome = Some(vec![ConfigOverlayMessage::ApplyConfigChanges(vec![
            ConfigChange {
                config_id: THEME_CONFIG_ID.to_string(),
                new_value: "   ".to_string(),
            },
        ])]);

        let effects = app.state.handle_config_overlay_messages(outcome);

        let actions = effects.unwrap_or_default();
        assert!(matches!(
            actions.as_slice(),
            [AppAction::SetTheme { file: None }]
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
        let outcome = Some(vec![ConfigOverlayMessage::ApplyConfigChanges(vec![
            ConfigChange {
                config_id: "model".to_string(),
                new_value: "gpt-5".to_string(),
            },
        ])]);

        let effects = app.state.handle_config_overlay_messages(outcome);

        let actions = effects.unwrap_or_default();
        assert!(matches!(
            actions.as_slice(),
            [AppAction::SetConfigOption {
                config_id,
                new_value
            }] if config_id == "model" && new_value == "gpt-5"
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

        let context = ViewContext::new((120, 40));
        let output = render_app(&mut screen, &context);
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

        let context = ViewContext::new((120, 40));
        let output = render_app(&mut screen, &context);
        assert!(
            output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );

        screen.state.config_overlay = None;
        let output = render_app(&mut screen, &context);
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

        let output = render_app(&mut app, &ViewContext::new((120, 40)));
        assert!(
            !output
                .lines()
                .iter()
                .any(|line| line.plain_text().contains("Plan"))
        );
    }

    #[test]
    fn plan_version_increments_on_replace() {
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );

        let initial_version = app.state.plan_tracker.version();
        app.state.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "Task A",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Pending,
            )],
            Instant::now(),
        );

        assert!(app.state.plan_tracker.version() > initial_version);
    }

    #[test]
    fn plan_version_increments_on_clear() {
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
                "Task A",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Pending,
            )],
            Instant::now(),
        );
        let version_before_clear = app.state.plan_tracker.version();
        app.state.plan_tracker.clear();

        assert!(app.state.plan_tracker.version() > version_before_clear);
    }

    #[test]
    fn props_include_plan_version_not_count() {
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );

        app.state.plan_tracker.replace(
            vec![
                acp::PlanEntry::new(
                    "Task A",
                    acp::PlanEntryPriority::Medium,
                    acp::PlanEntryStatus::Pending,
                ),
                acp::PlanEntry::new(
                    "Task B",
                    acp::PlanEntryPriority::Medium,
                    acp::PlanEntryStatus::Pending,
                ),
            ],
            Instant::now(),
        );

        let context = ViewContext::new((120, 40));
        app.prepare_for_view(&context);

        assert_eq!(app.cached_plan_version, app.state.plan_tracker.version());
    }

    #[test]
    fn tool_tick_advances_even_when_grid_loader_hidden() {
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );

        let tool_call = acp::ToolCall::new("tool-1".to_string(), "test_tool");
        app.state.tool_call_statuses.on_tool_call(&tool_call);
        app.state.grid_loader.visible = false;

        let tick_before = app.state.tool_call_statuses.tick();
        app.state.on_event(&Event::Tick);
        let tick_after = app.state.tool_call_statuses.tick();

        assert!(tick_after > tick_before);
    }

    #[test]
    fn progress_tick_advances_when_tools_running() {
        let mut app = App::new(
            "test-agent".to_string(),
            &[],
            vec![],
            AcpPromptHandle::noop(),
            acp::SessionId::new("test"),
            PathBuf::from("."),
        );

        let tool_call = acp::ToolCall::new("tool-1".to_string(), "test_tool");
        app.state.tool_call_statuses.on_tool_call(&tool_call);

        app.state.progress_indicator.update(0, 1);
        let ctx = ViewContext::new((80, 24));
        let output_before = app.state.progress_indicator.render(&ctx);
        app.state.on_event(&Event::Tick);
        let output_after = app.state.progress_indicator.render(&ctx);

        assert_ne!(
            output_before[0].plain_text(),
            output_after[0].plain_text(),
            "spinner frame should change after tick"
        );
    }
}
