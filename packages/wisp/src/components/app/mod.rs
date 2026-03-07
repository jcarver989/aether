mod attachments;
mod session;

use crate::components::config_menu::ConfigMenu;
use crate::components::config_overlay::{ConfigOverlay, ConfigOverlayAction};
use crate::components::container::Container;
use crate::components::conversation_window::{ConversationBuffer, ConversationWindow};
use crate::components::elicitation_form::ElicitationForm;
use crate::components::file_picker::FileMatch;
use crate::components::plan_tracker::PlanTracker;
use crate::components::plan_view::PlanView;
use crate::components::progress_indicator::ProgressIndicator;
use crate::components::prompt_composer::{PromptComposer, PromptComposerAction};
use crate::components::server_status::server_status_summary;
use crate::components::status_line::StatusLine;
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::settings::{list_theme_files, load_or_create_settings};
use crate::tui::spinner::Spinner;
use crate::tui::{
    Cursor, CursorComponent, FormAction, HandlesInput, InputOutcome, Line, RenderContext,
    RenderOutput,
};
use acp_utils::config_option_id::{ConfigOptionId, THEME_CONFIG_ID};
use acp_utils::notifications::{McpServerStatus, McpServerStatusEntry};
use agent_client_protocol::{
    self as acp, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions,
};
use crossterm::event::{self, KeyCode, KeyEvent};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use utils::ReasoningEffort;

pub use attachments::build_attachment_blocks;

/// Grace period for completed plan entries before they disappear.
const COMPLETED_ENTRY_GRACE_PERIOD: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub enum AppEvent {
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
    tool_call_statuses: ToolCallStatuses,
    grid_loader: Spinner,
    conversation: ConversationBuffer,
    prompt_composer: PromptComposer,
    agent_name: String,
    mode_display: Option<String>,
    model_display: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    config_options: Vec<SessionConfigOption>,
    waiting_for_response: bool,
    animation_tick: u16,
    context_usage_pct: Option<u8>,
    config_overlay: Option<ConfigOverlay>,
    elicitation_form: Option<ElicitationForm>,
    server_statuses: Vec<McpServerStatusEntry>,
    auth_methods: Vec<acp::AuthMethod>,
    plan_tracker: PlanTracker,
}

impl App {
    pub fn new(
        agent_name: String,
        config_options: &[SessionConfigOption],
        auth_methods: Vec<acp::AuthMethod>,
    ) -> Self {
        Self {
            tool_call_statuses: ToolCallStatuses::new(),
            grid_loader: Spinner::default(),
            conversation: ConversationBuffer::new(),
            prompt_composer: PromptComposer::new(),
            agent_name,
            mode_display: extract_mode_display(config_options),
            model_display: extract_model_display(config_options),
            reasoning_effort: extract_reasoning_effort(config_options),
            config_options: config_options.to_vec(),
            waiting_for_response: false,
            animation_tick: 0,
            context_usage_pct: None,
            config_overlay: None,
            elicitation_form: None,
            server_statuses: Vec::new(),
            auth_methods,
            plan_tracker: PlanTracker::default(),
        }
    }

    pub fn on_key_event(&mut self, key_event: KeyEvent) -> Vec<AppEvent> {
        if key_event.code == KeyCode::Char('c')
            && key_event.modifiers.contains(event::KeyModifiers::CONTROL)
        {
            return vec![AppEvent::Exit];
        }

        if let Some(effects) = self.handle_elicitation_key(key_event) {
            return effects;
        }

        if let Some(ref mut overlay) = self.config_overlay {
            let outcome = overlay.handle_key(key_event);
            return self.handle_config_overlay_outcome(outcome);
        }

        let composer_outcome = self.prompt_composer.handle_key(key_event);
        if composer_outcome.consumed {
            return self.handle_prompt_composer_outcome(composer_outcome);
        }

        if key_event.code == KeyCode::BackTab {
            if let Some(cycle_event) = self.cycle_quick_option() {
                return vec![cycle_event, AppEvent::Render];
            }
            return vec![];
        }

        if key_event.code == KeyCode::Char('t')
            && key_event.modifiers.contains(event::KeyModifiers::ALT)
        {
            if let Some(cycle_event) = self.cycle_reasoning_option() {
                return vec![cycle_event, AppEvent::Render];
            }
            return vec![];
        }

        if key_event.code == KeyCode::Esc && self.waiting_for_response {
            return vec![AppEvent::Cancel];
        }

        vec![]
    }

    pub fn on_paste(&mut self, text: &str) -> Vec<AppEvent> {
        self.config_overlay = None;
        if self.prompt_composer.on_paste(text) {
            vec![AppEvent::Render]
        } else {
            vec![]
        }
    }

    pub fn on_resize(_cols: u16, _rows: u16) -> Vec<AppEvent> {
        vec![AppEvent::Render]
    }

    #[allow(dead_code)]
    pub fn has_file_picker(&self) -> bool {
        self.prompt_composer.has_file_picker()
    }

    #[allow(dead_code)]
    pub fn has_command_picker(&self) -> bool {
        self.prompt_composer.has_command_picker()
    }

    #[allow(dead_code)]
    pub fn has_config_overlay(&self) -> bool {
        self.config_overlay.is_some()
    }

    #[allow(dead_code)]
    pub fn has_config_menu(&self) -> bool {
        self.config_overlay.is_some()
    }

    #[allow(dead_code)]
    pub fn has_config_picker(&self) -> bool {
        self.config_overlay
            .as_ref()
            .is_some_and(ConfigOverlay::has_picker)
    }

    #[allow(dead_code)]
    pub fn config_menu_selected_index(&self) -> Option<usize> {
        self.config_overlay
            .as_ref()
            .map(ConfigOverlay::menu_selected_index)
    }

    #[allow(dead_code)]
    pub fn config_picker_config_id(&self) -> Option<&str> {
        self.config_overlay
            .as_ref()
            .and_then(|overlay| overlay.picker_config_id())
    }

    #[allow(dead_code)]
    pub fn file_picker_selected_display_name(&self) -> Option<String> {
        self.prompt_composer.file_picker_selected_display_name()
    }

    #[allow(dead_code)]
    pub fn command_picker_match_names(&self) -> Vec<&str> {
        self.prompt_composer.command_picker_match_names()
    }

    #[allow(dead_code)]
    pub fn open_file_picker_with_matches(&mut self, matches: Vec<FileMatch>) {
        self.prompt_composer.open_file_picker_with_matches(matches);
    }

    #[allow(dead_code)]
    pub fn available_commands(&self) -> &[crate::components::command_picker::CommandEntry] {
        self.prompt_composer.available_commands()
    }

    fn handle_elicitation_key(&mut self, key_event: KeyEvent) -> Option<Vec<AppEvent>> {
        let elicitation_form = self.elicitation_form.as_mut()?;
        let outcome = elicitation_form.form.handle_key(key_event);

        match outcome.action {
            Some(FormAction::Close) => {
                if let Some(elicitation_form) = self.elicitation_form.take() {
                    let _ = elicitation_form
                        .response_tx
                        .send(ElicitationForm::decline());
                }
            }
            Some(FormAction::Submit) => {
                if let Some(elicitation_form) = self.elicitation_form.take() {
                    let response = elicitation_form.confirm();
                    let _ = elicitation_form.response_tx.send(response);
                }
            }
            None => {}
        }

        if outcome.needs_render {
            Some(vec![AppEvent::Render])
        } else {
            Some(vec![])
        }
    }

    fn handle_prompt_composer_outcome(
        &mut self,
        outcome: InputOutcome<PromptComposerAction>,
    ) -> Vec<AppEvent> {
        match outcome.action {
            Some(PromptComposerAction::OpenConfig) => {
                self.open_config_overlay();
                vec![AppEvent::Render]
            }
            Some(PromptComposerAction::SubmitRequested {
                user_input,
                attachments,
            }) => {
                let mut effects = vec![
                    AppEvent::PushScrollback(vec![Line::new(String::new())]),
                    AppEvent::PushScrollback(vec![Line::new(user_input.clone())]),
                ];

                self.waiting_for_response = true;
                self.animation_tick = 0;
                self.grid_loader.visible = true;
                self.grid_loader.tick = 0;

                effects.push(AppEvent::Render);
                effects.push(AppEvent::PromptSubmit {
                    user_input,
                    attachments,
                });
                effects
            }
            None if outcome.needs_render => vec![AppEvent::Render],
            None => vec![],
        }
    }

    fn handle_config_overlay_outcome(
        &mut self,
        outcome: InputOutcome<ConfigOverlayAction>,
    ) -> Vec<AppEvent> {
        match outcome.action {
            Some(ConfigOverlayAction::Close) => {
                self.config_overlay = None;
                vec![AppEvent::Render]
            }
            Some(ConfigOverlayAction::ApplyConfigChanges(changes)) => {
                let mut events = Vec::new();
                for change in changes {
                    if change.config_id == THEME_CONFIG_ID {
                        events.push(AppEvent::SetTheme {
                            file: theme_file_from_picker_value(&change.new_value),
                        });
                    } else {
                        events.push(AppEvent::SetConfigOption {
                            config_id: change.config_id,
                            new_value: change.new_value,
                        });
                    }
                }
                events.push(AppEvent::Render);
                events
            }
            Some(ConfigOverlayAction::AuthenticateServer(name)) => {
                vec![
                    AppEvent::AuthenticateMcpServer { server_name: name },
                    AppEvent::Render,
                ]
            }
            Some(ConfigOverlayAction::AuthenticateProvider(method_id)) => {
                vec![
                    AppEvent::AuthenticateProvider { method_id },
                    AppEvent::Render,
                ]
            }
            None if outcome.needs_render => vec![AppEvent::Render],
            None => vec![],
        }
    }

    fn cycle_quick_option(&self) -> Option<AppEvent> {
        let option = self
            .config_options
            .iter()
            .find(|option| is_cycleable_mode_option(option))?;

        let SessionConfigKind::Select(ref select) = option.kind else {
            return None;
        };

        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            return None;
        };

        if options.is_empty() {
            return None;
        }

        let current_index = options
            .iter()
            .position(|entry| entry.value == select.current_value)
            .unwrap_or(0);
        let next_index = (current_index + 1) % options.len();
        let next_value = options.get(next_index)?.value.0.to_string();

        Some(AppEvent::SetConfigOption {
            config_id: option.id.0.to_string(),
            new_value: next_value,
        })
    }

    fn cycle_reasoning_option(&self) -> Option<AppEvent> {
        self.config_options
            .iter()
            .any(|option| option.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str())
            .then(|| {
                let next = ReasoningEffort::cycle_next(self.reasoning_effort);
                AppEvent::SetConfigOption {
                    config_id: ConfigOptionId::ReasoningEffort.as_str().to_string(),
                    new_value: ReasoningEffort::config_str(next).to_string(),
                }
            })
    }

    fn open_config_overlay(&mut self) {
        let menu = ConfigMenu::from_config_options(&self.config_options);
        let menu = self.decorate_config_menu(menu);
        self.config_overlay = Some(
            ConfigOverlay::new(
                menu,
                self.server_statuses.clone(),
                self.auth_methods.clone(),
            )
            .with_reasoning_effort_from_options(&self.config_options),
        );
    }

    fn decorate_config_menu(&self, mut menu: ConfigMenu) -> ConfigMenu {
        let settings = load_or_create_settings();
        let theme_files = list_theme_files();
        menu.add_theme_entry(settings.theme.file.as_deref(), &theme_files);

        let server_summary = server_status_summary(&self.server_statuses);
        menu.add_mcp_servers_entry(&server_summary);
        if !self.auth_methods.is_empty() {
            let summary = format!("{} needs login", self.auth_methods.len());
            menu.add_provider_logins_entry(&summary);
        }
        menu
    }
}

impl CursorComponent for App {
    fn render_with_cursor(&mut self, context: &RenderContext) -> RenderOutput {
        let unhealthy_count = self
            .server_statuses
            .iter()
            .filter(|status| !matches!(status.status, McpServerStatus::Connected { .. }))
            .count();
        let mut status_line = StatusLine {
            agent_name: &self.agent_name,
            mode_display: self.mode_display.as_deref(),
            model_display: self.model_display.as_deref(),
            reasoning_effort: self.reasoning_effort,
            context_pct_left: self.context_usage_pct,
            waiting_for_response: self.waiting_for_response,
            unhealthy_server_count: unhealthy_count,
        };

        if let Some(ref mut overlay) = self.config_overlay {
            let cursor = Cursor {
                logical_row: overlay.cursor_row_offset(),
                col: overlay.cursor_col(),
            };

            let mut container = Container::new(vec![overlay, &mut status_line]);
            let (lines, _) = container.render_with_offsets(context);

            return RenderOutput {
                lines,
                cursor,
                cursor_visible: overlay.has_picker(),
            };
        }

        let visible_plan_entries = self
            .plan_tracker
            .visible_entries(Instant::now(), COMPLETED_ENTRY_GRACE_PERIOD);

        let mut conversation_window = ConversationWindow {
            loader: &mut self.grid_loader,
            conversation: &mut self.conversation,
            tool_call_statuses: &self.tool_call_statuses,
        };
        let mut plan_view = PlanView {
            entries: &visible_plan_entries,
        };
        let progress = self.tool_call_statuses.progress();
        let mut progress_indicator = ProgressIndicator {
            completed: progress.completed_top_level,
            total: progress.total_top_level,
            tick: self.animation_tick,
        };

        let mut container: Container<'_> = Container::new(vec![
            &mut conversation_window,
            &mut plan_view,
            &mut progress_indicator,
            &mut self.prompt_composer,
        ]);
        let prompt_component_index = container.len() - 1;

        if let Some(ref mut elicitation_form) = self.elicitation_form {
            container.push(&mut elicitation_form.form);
        }

        container.push(&mut status_line);
        let (lines, offsets) = container.render_with_offsets(context);
        let prompt_cursor = self.prompt_composer.cursor(context);
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

fn is_cycleable_mode_option(option: &SessionConfigOption) -> bool {
    matches!(option.kind, SessionConfigKind::Select(_))
        && option.category == Some(SessionConfigOptionCategory::Mode)
}

fn option_display_name(
    options: &SessionConfigSelectOptions,
    current_value: &acp::SessionConfigValueId,
) -> Option<String> {
    match options {
        SessionConfigSelectOptions::Ungrouped(options) => options
            .iter()
            .find(|option| &option.value == current_value)
            .map(|option| option.name.clone()),
        SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| group.options.iter())
            .find(|option| &option.value == current_value)
            .map(|option| option.name.clone()),
        _ => None,
    }
}

fn extract_select_display(
    config_options: &[SessionConfigOption],
    id: ConfigOptionId,
) -> Option<String> {
    let option = config_options
        .iter()
        .find(|option| option.id.0.as_ref() == id.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    option_display_name(&select.options, &select.current_value)
}

fn extract_mode_display(config_options: &[SessionConfigOption]) -> Option<String> {
    extract_select_display(config_options, ConfigOptionId::Mode)
}

fn theme_file_from_picker_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn extract_model_display(config_options: &[SessionConfigOption]) -> Option<String> {
    let option = config_options
        .iter()
        .find(|option| option.id.0.as_ref() == ConfigOptionId::Model.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    let options = match &select.options {
        SessionConfigSelectOptions::Ungrouped(options) => options,
        SessionConfigSelectOptions::Grouped(_) => {
            return extract_select_display(config_options, ConfigOptionId::Model);
        }
        _ => return None,
    };

    let current = select.current_value.0.as_ref();
    if current.contains(',') {
        let names: Vec<&str> = current
            .split(',')
            .filter_map(|part| {
                let trimmed = part.trim();
                options
                    .iter()
                    .find(|option| option.value.0.as_ref() == trimmed)
                    .map(|option| option.name.as_str())
            })
            .collect();
        if names.is_empty() {
            None
        } else {
            Some(names.join(" + "))
        }
    } else {
        extract_select_display(config_options, ConfigOptionId::Model)
    }
}

fn extract_reasoning_effort(config_options: &[SessionConfigOption]) -> Option<ReasoningEffort> {
    let option = config_options
        .iter()
        .find(|option| option.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    ReasoningEffort::parse(&select.current_value.0).unwrap_or(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::command_picker::CommandEntry;
    use crate::settings::{ThemeSettings as WispThemeSettings, WispSettings, save_settings};
    use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
    use crossterm::event::KeyModifiers;
    use std::fs;
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
            theme = crate::tui::theme::Theme::load(&settings);
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
            let menu = app.decorate_config_menu(ConfigMenu::from_config_options(&[]));

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
            let menu = app.decorate_config_menu(ConfigMenu::from_config_options(&[]));
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
            InputOutcome::action_and_render(ConfigOverlayAction::ApplyConfigChanges(vec![
                crate::components::config_menu::ConfigChange {
                    config_id: THEME_CONFIG_ID.to_string(),
                    new_value: "catppuccin.tmTheme".to_string(),
                },
            ]));

        let effects = app.handle_config_overlay_outcome(outcome);

        assert!(matches!(
            effects.as_slice(),
            [
                AppEvent::SetTheme {
                    file: Some(file)
                },
                AppEvent::Render
            ] if file == "catppuccin.tmTheme"
        ));
    }

    #[test]
    fn theme_default_value_maps_to_none() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let outcome =
            InputOutcome::action_and_render(ConfigOverlayAction::ApplyConfigChanges(vec![
                crate::components::config_menu::ConfigChange {
                    config_id: THEME_CONFIG_ID.to_string(),
                    new_value: "   ".to_string(),
                },
            ]));

        let effects = app.handle_config_overlay_outcome(outcome);

        assert!(matches!(
            effects.as_slice(),
            [AppEvent::SetTheme { file: None }, AppEvent::Render]
        ));
    }

    #[test]
    fn non_theme_config_change_still_emits_set_config_option() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let outcome =
            InputOutcome::action_and_render(ConfigOverlayAction::ApplyConfigChanges(vec![
                crate::components::config_menu::ConfigChange {
                    config_id: "model".to_string(),
                    new_value: "gpt-5".to_string(),
                },
            ]));

        let effects = app.handle_config_overlay_outcome(outcome);

        assert!(matches!(
            effects.as_slice(),
            [
                AppEvent::SetConfigOption {
                    config_id,
                    new_value
                },
                AppEvent::Render
            ] if config_id == "model" && new_value == "gpt-5"
        ));
    }

    #[test]
    fn command_picker_cursor_stays_in_input_prompt() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen
            .prompt_composer
            .open_command_picker_with_entries(vec![CommandEntry {
                name: "config".to_string(),
                description: "Open config".to_string(),
                has_input: false,
                hint: None,
                builtin: true,
            }]);

        let context = RenderContext::new((120, 40));
        let output = screen.render_with_cursor(&context);
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
        screen.open_config_overlay();

        let context = RenderContext::new((120, 40));
        let output = screen.render_with_cursor(&context);
        assert!(
            output
                .lines
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );

        screen.config_overlay = None;
        let output = screen.render_with_cursor(&context);
        assert!(
            !output
                .lines
                .iter()
                .any(|line| line.plain_text().contains("Configuration"))
        );
    }

    #[test]
    fn shift_tab_cycles_mode_option() {
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
        let effects = app.on_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));

        assert!(effects.iter().any(|event| {
            matches!(
                event,
                AppEvent::SetConfigOption { config_id, new_value }
                if config_id == "mode" && new_value == "Coder"
            )
        }));
    }

    #[test]
    fn shift_tab_wraps_mode_option() {
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
        let effects = app.on_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));

        assert!(effects.iter().any(|event| {
            matches!(
                event,
                AppEvent::SetConfigOption { config_id, new_value }
                if config_id == "mode" && new_value == "Planner"
            )
        }));
    }

    #[test]
    fn shift_tab_ignored_when_overlay_consumes_input() {
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
        app.open_config_overlay();

        let effects = app.on_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert!(
            !effects
                .iter()
                .any(|event| matches!(event, AppEvent::SetConfigOption { .. }))
        );
    }

    #[test]
    fn shift_tab_noop_when_no_cycleable_option_exists() {
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
        let effects = app.on_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert!(effects.is_empty());
    }

    #[test]
    fn ctrl_c_emits_exit() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        let effects = screen.on_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(matches!(effects.as_slice(), [AppEvent::Exit]));
    }

    #[test]
    fn escape_while_waiting_emits_cancel() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen.waiting_for_response = true;

        let effects = screen.on_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(effects.as_slice(), [AppEvent::Cancel]));
    }

    #[test]
    fn escape_while_not_waiting_does_nothing() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen.waiting_for_response = false;

        let effects = screen.on_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(effects.is_empty());
    }

    #[test]
    fn extract_model_display_handles_comma_separated_value() {
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
        app.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "1",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Completed,
            )],
            Instant::now()
                .checked_sub(COMPLETED_ENTRY_GRACE_PERIOD + Duration::from_millis(1))
                .unwrap(),
        );

        let output = app.render_with_cursor(&RenderContext::new((120, 40)));
        assert!(
            !output
                .lines
                .iter()
                .any(|line| line.plain_text().contains("Plan"))
        );
    }
}
