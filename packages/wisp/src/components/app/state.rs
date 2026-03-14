use super::{AppAction, GitDiffMode, ScreenMode, theme_file_from_picker_value};
use crate::components::config_menu::ConfigMenu;
use crate::components::config_overlay::{ConfigOverlay, ConfigOverlayMessage};
use crate::components::conversation_window::ConversationBuffer;
use crate::components::elicitation_form::ElicitationForm;
use crate::components::plan_tracker::PlanTracker;
use crate::components::progress_indicator::ProgressIndicator;
use crate::components::prompt_composer::{PromptComposer, PromptComposerMessage};
use crate::components::server_status::server_status_summary;
use crate::components::session_picker::{SessionEntry, SessionPicker};
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::keybindings::Keybindings;
use crate::settings::{list_theme_files, load_or_create_settings};
use crate::tui::{
    Component, Event, FocusRing, FormMessage, KeyEvent, Line, PickerMessage, Spinner, ViewContext,
};
use acp_utils::config_option_id::{ConfigOptionId, THEME_CONFIG_ID};
use acp_utils::notifications::McpServerStatusEntry;
use agent_client_protocol::{
    self as acp, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions,
};
use std::time::Instant;
use utils::ReasoningEffort;

pub(super) const FOCUS_COMPOSER: usize = 0;
pub(super) const FOCUS_CONFIG_OVERLAY: usize = 1;
pub(super) const FOCUS_ELICITATION: usize = 2;

pub struct UiState {
    pub(crate) tool_call_statuses: ToolCallStatuses,
    pub(crate) grid_loader: Spinner,
    pub(crate) conversation: ConversationBuffer,
    pub(crate) prompt_composer: PromptComposer,
    pub(crate) agent_name: String,
    pub(crate) mode_display: Option<String>,
    pub(crate) model_display: Option<String>,
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
    pub(crate) config_options: Vec<SessionConfigOption>,
    pub(crate) waiting_for_response: bool,
    pub(crate) context_usage_pct: Option<u8>,
    pub(crate) progress_indicator: ProgressIndicator,
    pub(crate) config_overlay: Option<ConfigOverlay>,
    pub(crate) elicitation_form: Option<ElicitationForm>,
    pub(crate) session_picker: Option<SessionPicker>,
    pub(crate) server_statuses: Vec<McpServerStatusEntry>,
    pub(crate) auth_methods: Vec<acp::AuthMethod>,
    pub(crate) plan_tracker: PlanTracker,
    pub(crate) keybindings: Keybindings,
    pub(crate) screen_mode: ScreenMode,
    pub(crate) exit_requested: bool,
    pub(super) focus: FocusRing,
}

impl UiState {
    pub fn new(
        agent_name: String,
        config_options: &[SessionConfigOption],
        auth_methods: Vec<acp::AuthMethod>,
    ) -> Self {
        let keybindings = Keybindings::default();
        Self {
            tool_call_statuses: ToolCallStatuses::new(),
            grid_loader: Spinner::default(),
            conversation: ConversationBuffer::new(),
            prompt_composer: PromptComposer::new(keybindings.clone()),
            agent_name,
            mode_display: extract_mode_display(config_options),
            model_display: extract_model_display(config_options),
            reasoning_effort: extract_reasoning_effort(config_options),
            config_options: config_options.to_vec(),
            waiting_for_response: false,
            context_usage_pct: None,
            progress_indicator: ProgressIndicator::default(),
            config_overlay: None,
            elicitation_form: None,
            session_picker: None,
            server_statuses: Vec::new(),
            auth_methods,
            plan_tracker: PlanTracker::default(),
            keybindings,
            screen_mode: ScreenMode::Conversation,
            exit_requested: false,
            focus: FocusRing::new(3),
        }
    }

    pub(crate) fn enter_git_diff(&mut self) {
        self.screen_mode = ScreenMode::GitDiff;
    }

    pub(crate) fn exit_git_diff(&mut self) {
        self.screen_mode = ScreenMode::Conversation;
    }

    pub(crate) fn wants_tick(&self) -> bool {
        self.grid_loader.visible
            || self.tool_call_statuses.progress().running_any
            || self.plan_tracker_has_tick_driven_visibility()
    }

    fn plan_tracker_has_tick_driven_visibility(&self) -> bool {
        self.plan_tracker
            .visible_entries(
                self.plan_tracker.last_tick(),
                self.plan_tracker.grace_period,
            )
            .iter()
            .any(|entry| matches!(entry.status, acp::PlanEntryStatus::Completed))
    }

    pub(crate) fn on_event(&mut self, event: &Event) -> Option<Vec<AppAction>> {
        match event {
            Event::Key(key_event) => self.handle_key(*key_event),
            Event::Paste(_) => {
                self.config_overlay = None;
                let outcome = self.prompt_composer.on_event(event);
                Some(self.handle_prompt_composer_messages(outcome))
            }
            Event::Tick => {
                let now = Instant::now();
                self.grid_loader.on_tick();
                self.tool_call_statuses.on_tick(now);
                self.plan_tracker.on_tick(now);
                self.progress_indicator.on_tick();
                Some(vec![])
            }
            Event::Mouse(_) | Event::Resize(_) => Some(vec![]),
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) -> Option<Vec<AppAction>> {
        if self.keybindings.exit.matches(key_event) {
            self.exit_requested = true;
            return Some(vec![]);
        }

        // Elicitation form captures all input when present
        if self.elicitation_form.is_some() {
            return self
                .handle_elicitation_key(key_event)
                .unwrap_or(Some(vec![]));
        }

        // Session picker captures all input when present
        if self.session_picker.is_some() {
            return self.handle_session_picker_key(key_event);
        }

        if self.keybindings.toggle_git_diff.matches(key_event) {
            let close_git_diff = matches!(self.screen_mode, ScreenMode::GitDiff);
            if close_git_diff {
                self.screen_mode = ScreenMode::Conversation;
            }
            return if close_git_diff {
                Some(vec![])
            } else {
                Some(vec![AppAction::OpenGitDiffViewer])
            };
        }

        let event = Event::Key(key_event);

        if self.focus.focused() == FOCUS_CONFIG_OVERLAY {
            let outcome = self
                .config_overlay
                .as_mut()
                .expect("config overlay")
                .on_event(&event);
            Some(self.handle_config_overlay_messages(outcome))
        } else {
            if matches!(self.screen_mode, ScreenMode::GitDiff) {
                return Some(vec![]);
            }

            let composer_outcome = self.prompt_composer.on_event(&event);
            if composer_outcome.is_some() {
                return Some(self.handle_prompt_composer_messages(composer_outcome));
            }

            if self.keybindings.cycle_reasoning.matches(key_event) {
                return self
                    .cycle_reasoning_option()
                    .map_or(Some(vec![]), |action| Some(vec![action]));
            }

            if self.keybindings.cycle_mode.matches(key_event) {
                return self
                    .cycle_quick_option()
                    .map_or(Some(vec![]), |action| Some(vec![action]));
            }

            if self.keybindings.cancel.matches(key_event) && self.waiting_for_response {
                return Some(vec![AppAction::Cancel]);
            }

            Some(vec![])
        }
    }

    pub(crate) fn update_config_options(&mut self, config_options: &[SessionConfigOption]) {
        self.mode_display = extract_mode_display(config_options);
        self.model_display = extract_model_display(config_options);
        self.reasoning_effort = extract_reasoning_effort(config_options);
        self.config_options = config_options.to_vec();
    }

    pub(crate) fn cycle_quick_option(&self) -> Option<AppAction> {
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

        Some(AppAction::SetConfigOption {
            config_id: option.id.0.to_string(),
            new_value: next_value,
        })
    }

    pub(crate) fn cycle_reasoning_option(&self) -> Option<AppAction> {
        self.config_options
            .iter()
            .any(|option| option.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str())
            .then(|| {
                let next = ReasoningEffort::cycle_next(self.reasoning_effort);
                AppAction::SetConfigOption {
                    config_id: ConfigOptionId::ReasoningEffort.as_str().to_string(),
                    new_value: ReasoningEffort::config_str(next).to_string(),
                }
            })
    }

    #[allow(clippy::option_option)]
    pub(crate) fn handle_elicitation_key(
        &mut self,
        key_event: KeyEvent,
    ) -> Option<Option<Vec<AppAction>>> {
        let elicitation_form = self.elicitation_form.as_mut()?;
        let outcome = elicitation_form.form.on_event(&Event::Key(key_event));

        for message in outcome.unwrap_or_default() {
            match message {
                FormMessage::Close => {
                    if let Some(elicitation_form) = self.elicitation_form.take() {
                        let _ = elicitation_form
                            .response_tx
                            .send(ElicitationForm::decline());
                    }
                    self.focus.focus(FOCUS_COMPOSER);
                }
                FormMessage::Submit => {
                    if let Some(elicitation_form) = self.elicitation_form.take() {
                        let response = elicitation_form.confirm();
                        let _ = elicitation_form.response_tx.send(response);
                    }
                    self.focus.focus(FOCUS_COMPOSER);
                }
            }
        }

        Some(Some(vec![]))
    }

    pub(crate) fn handle_prompt_composer_messages(
        &mut self,
        outcome: Option<Vec<PromptComposerMessage>>,
    ) -> Vec<AppAction> {
        outcome
            .unwrap_or_default()
            .into_iter()
            .flat_map(|msg| match msg {
                PromptComposerMessage::ClearScreen => {
                    vec![AppAction::ClearScreen]
                }
                PromptComposerMessage::OpenConfig => {
                    self.open_config_overlay();
                    vec![]
                }
                PromptComposerMessage::OpenSessionPicker => {
                    vec![AppAction::ListSessions]
                }
                PromptComposerMessage::SubmitRequested {
                    user_input,
                    attachments,
                } => {
                    self.waiting_for_response = true;
                    self.grid_loader.reset();
                    vec![
                        AppAction::PushScrollback(vec![Line::new(String::new())]),
                        AppAction::PushScrollback(vec![Line::new(user_input.clone())]),
                        AppAction::PromptSubmit {
                            user_input,
                            attachments,
                        },
                    ]
                }
            })
            .collect()
    }

    pub(crate) fn handle_config_overlay_messages(
        &mut self,
        outcome: Option<Vec<ConfigOverlayMessage>>,
    ) -> Vec<AppAction> {
        outcome
            .unwrap_or_default()
            .into_iter()
            .flat_map(|message| match message {
                ConfigOverlayMessage::Close => {
                    self.config_overlay = None;
                    self.focus.focus(FOCUS_COMPOSER);
                    vec![]
                }
                ConfigOverlayMessage::ApplyConfigChanges(changes) => changes
                    .into_iter()
                    .map(|change| {
                        if change.config_id == THEME_CONFIG_ID {
                            AppAction::SetTheme {
                                file: theme_file_from_picker_value(&change.new_value),
                            }
                        } else {
                            AppAction::SetConfigOption {
                                config_id: change.config_id,
                                new_value: change.new_value,
                            }
                        }
                    })
                    .collect(),
                ConfigOverlayMessage::AuthenticateServer(name) => {
                    vec![AppAction::AuthenticateMcpServer { server_name: name }]
                }
                ConfigOverlayMessage::AuthenticateProvider(method_id) => {
                    vec![AppAction::AuthenticateProvider { method_id }]
                }
            })
            .collect()
    }

    pub(crate) fn open_config_overlay(&mut self) {
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
        self.focus.focus(FOCUS_CONFIG_OVERLAY);
    }

    pub(crate) fn open_session_picker(&mut self, sessions: Vec<acp::SessionInfo>) {
        let entries = sessions.into_iter().map(SessionEntry).collect();
        self.session_picker = Some(SessionPicker::new(entries));
    }

    fn handle_session_picker_key(&mut self, key_event: KeyEvent) -> Option<Vec<AppAction>> {
        let picker = self.session_picker.as_mut()?;
        let msgs = picker.on_event(&Event::Key(key_event)).unwrap_or_default();
        for msg in msgs {
            match msg {
                PickerMessage::Close => {
                    self.session_picker = None;
                }
                PickerMessage::Confirm(entry) => {
                    self.session_picker = None;
                    let info = entry.0;
                    return Some(vec![AppAction::LoadSession {
                        session_id: info.session_id.0.to_string(),
                        cwd: info.cwd,
                    }]);
                }
                _ => {}
            }
        }
        Some(vec![])
    }

    pub(crate) fn decorate_config_menu(&self, mut menu: ConfigMenu) -> ConfigMenu {
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

    pub(crate) fn refresh_caches(
        &mut self,
        context: &ViewContext,
        git_diff_mode: Option<&mut GitDiffMode>,
    ) {
        let progress = self.tool_call_statuses.progress();
        self.progress_indicator
            .update(progress.completed_top_level, progress.total_top_level);
        self.conversation
            .ensure_all_rendered(&self.tool_call_statuses, context);

        if matches!(self.screen_mode, ScreenMode::GitDiff)
            && let Some(mode) = git_diff_mode
        {
            mode.refresh_caches(context);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn on_authenticate_started(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.on_authenticate_started(method_id);
        }
    }
}

pub(super) fn is_cycleable_mode_option(option: &SessionConfigOption) -> bool {
    matches!(option.kind, SessionConfigKind::Select(_))
        && option.category == Some(SessionConfigOptionCategory::Mode)
}

pub(super) fn option_display_name(
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

pub(super) fn extract_select_display(
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

pub(super) fn extract_mode_display(config_options: &[SessionConfigOption]) -> Option<String> {
    extract_select_display(config_options, ConfigOptionId::Mode)
}

pub(super) fn extract_model_display(config_options: &[SessionConfigOption]) -> Option<String> {
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

pub(super) fn extract_reasoning_effort(
    config_options: &[SessionConfigOption],
) -> Option<ReasoningEffort> {
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
    use crate::components::config_menu::ConfigChange;
    use crate::keybindings::KeyBinding;
    use crate::tui::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
    use agent_client_protocol::SessionConfigOptionCategory;

    #[test]
    fn ui_state_new_initializes_derived_displays() {
        let config_options = vec![
            SessionConfigOption::select(
                "mode",
                "Mode",
                "planner",
                vec![
                    acp::SessionConfigSelectOption::new("planner", "Planner"),
                    acp::SessionConfigSelectOption::new("coder", "Coder"),
                ],
            )
            .category(SessionConfigOptionCategory::Mode),
            SessionConfigOption::select(
                "model",
                "Model",
                "a:x,b:y",
                vec![
                    acp::SessionConfigSelectOption::new("a:x", "Alpha / X"),
                    acp::SessionConfigSelectOption::new("b:y", "Beta / Y"),
                ],
            )
            .category(SessionConfigOptionCategory::Model),
            SessionConfigOption::select(
                ConfigOptionId::ReasoningEffort.as_str(),
                "Reasoning",
                "high",
                vec![
                    acp::SessionConfigSelectOption::new("none", "None"),
                    acp::SessionConfigSelectOption::new("high", "High"),
                ],
            ),
        ];

        let state = UiState::new(
            "test-agent".to_string(),
            &config_options,
            vec![acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
                "anthropic",
                "Anthropic",
            ))],
        );

        assert_eq!(state.agent_name, "test-agent");
        assert_eq!(state.mode_display.as_deref(), Some("Planner"));
        assert_eq!(state.model_display.as_deref(), Some("Alpha / X + Beta / Y"));
        assert_eq!(state.reasoning_effort, Some(ReasoningEffort::High));
        assert_eq!(state.config_options.len(), 3);
        assert_eq!(state.auth_methods.len(), 1);
    }

    #[test]
    fn ui_state_new_initializes_defaults() {
        let state = UiState::new("test-agent".to_string(), &[], vec![]);

        assert!(!state.waiting_for_response);
        assert_eq!(state.context_usage_pct, None);
        assert!(state.config_overlay.is_none());
        assert!(state.elicitation_form.is_none());
        assert!(state.server_statuses.is_empty());
    }

    #[test]
    fn prompt_composer_messages_process_all_messages_in_order() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let outcome = Some(vec![
            PromptComposerMessage::OpenConfig,
            PromptComposerMessage::SubmitRequested {
                user_input: "hello".to_string(),
                attachments: vec![],
            },
        ]);

        let effects = state.handle_prompt_composer_messages(outcome);

        assert!(
            state.config_overlay.is_some(),
            "config overlay should be opened"
        );
        assert!(
            state.waiting_for_response,
            "submit should mark waiting state"
        );
        assert!(effects.iter().any(|e| matches!(
            e,
            AppAction::PromptSubmit { user_input, .. } if user_input == "hello"
        )));
    }

    #[test]
    fn config_overlay_messages_process_all_messages_in_order() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        state.open_config_overlay();
        let outcome = Some(vec![
            ConfigOverlayMessage::ApplyConfigChanges(vec![ConfigChange {
                config_id: "model".to_string(),
                new_value: "gpt-5".to_string(),
            }]),
            ConfigOverlayMessage::Close,
        ]);

        let effects = state.handle_config_overlay_messages(outcome);

        assert!(
            state.config_overlay.is_none(),
            "close message should be applied"
        );
        assert!(effects.iter().any(|e| matches!(
            e,
            AppAction::SetConfigOption { config_id, new_value }
                if config_id == "model" && new_value == "gpt-5"
        )));
    }

    #[test]
    fn handled_prompt_composer_result_returns_no_effects() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);

        let effects = state.handle_prompt_composer_messages(Some(vec![]));

        assert!(effects.is_empty());
    }

    #[test]
    fn custom_exit_keybinding_triggers_exit() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        state.keybindings.exit = KeyBinding::new(KeyCode::Char('q'), KeyModifiers::CONTROL);

        let default_exit = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        state.on_event(&Event::Key(default_exit));
        assert!(
            !state.exit_requested,
            "default Ctrl+C should no longer exit"
        );

        state.exit_requested = false;
        let custom_exit = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        state.on_event(&Event::Key(custom_exit));
        assert!(state.exit_requested, "custom Ctrl+Q should exit");
    }

    #[test]
    fn ctrl_g_opens_git_diff_viewer() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        let effects = state.on_event(&Event::Key(key));
        assert!(
            effects
                .unwrap_or_default()
                .iter()
                .any(|e| matches!(e, AppAction::OpenGitDiffViewer))
        );
    }

    #[test]
    fn ctrl_g_closes_git_diff_viewer() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        state.screen_mode = ScreenMode::GitDiff;

        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        let effects = state.on_event(&Event::Key(key));

        assert!(matches!(state.screen_mode, ScreenMode::Conversation));
        assert!(effects.unwrap_or_default().is_empty());
    }

    #[test]
    fn ctrl_g_blocked_during_elicitation() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        state.elicitation_form = Some(
            crate::components::elicitation_form::ElicitationForm::from_params(
                acp_utils::notifications::ElicitationParams {
                    message: "test".to_string(),
                    schema: acp_utils::ElicitationSchema::builder().build().unwrap(),
                },
                tokio::sync::oneshot::channel().0,
            ),
        );

        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        let effects = state.on_event(&Event::Key(key));

        assert!(
            !effects
                .unwrap_or_default()
                .iter()
                .any(|e| matches!(e, AppAction::OpenGitDiffViewer))
        );
    }

    #[test]
    fn esc_in_diff_mode_does_not_cancel() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        state.waiting_for_response = true;
        state.screen_mode = ScreenMode::GitDiff;

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let effects = state.on_event(&Event::Key(key));

        assert!(!state.exit_requested);
        assert!(
            !effects
                .unwrap_or_default()
                .iter()
                .any(|e| matches!(e, AppAction::Cancel)),
            "Esc should NOT cancel a running prompt while git diff mode is active"
        );
    }

    #[test]
    fn mouse_scroll_ignored_in_conversation_mode() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);

        let mouse = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        state.on_event(&Event::Mouse(mouse));
    }
}
