use super::{GitDiffMode, ScreenMode};
use crate::components::config_menu::ConfigMenu;
use crate::components::config_overlay::ConfigOverlay;
use crate::components::conversation_window::ConversationBuffer;
use crate::components::elicitation_form::ElicitationForm;
use crate::components::plan_tracker::PlanTracker;
use crate::components::progress_indicator::ProgressIndicator;
use crate::components::prompt_composer::PromptComposer;
use crate::components::server_status::server_status_summary;
use crate::components::session_picker::{SessionEntry, SessionPicker};
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::keybindings::Keybindings;
use crate::settings::{list_theme_files, load_or_create_settings};
use crate::tui::{FocusRing, Spinner, ViewContext};
use acp_utils::config_option_id::ConfigOptionId;
use acp_utils::notifications::McpServerStatusEntry;
use agent_client_protocol::{
    self as acp, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions,
};
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
    pub(crate) screen_mode: ScreenMode,
    pub exit_requested: bool,
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

    pub fn wants_tick(&self) -> bool {
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

    pub(crate) fn update_config_options(&mut self, config_options: &[SessionConfigOption]) {
        self.mode_display = extract_mode_display(config_options);
        self.model_display = extract_model_display(config_options);
        self.reasoning_effort = extract_reasoning_effort(config_options);
        self.config_options = config_options.to_vec();
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

    pub(crate) fn reset_after_context_cleared(&mut self) {
        self.conversation.clear();
        self.tool_call_statuses.clear();
        self.grid_loader.visible = false;
        self.waiting_for_response = false;
        self.context_usage_pct = None;
        self.plan_tracker.clear();
        self.progress_indicator = ProgressIndicator::default();
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
}
