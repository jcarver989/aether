use super::{GitDiffMode, ScreenMode};
use crate::components::config_menu::ConfigMenu;
use crate::components::config_overlay::ConfigOverlay;
use crate::components::conversation_window::{ConversationBuffer, SegmentContent};
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
use acp_utils::notifications::McpServerStatusEntry;
use agent_client_protocol::{self as acp, SessionConfigOption};
use std::path::PathBuf;

pub(super) const FOCUS_COMPOSER: usize = 0;
pub(super) const FOCUS_CONFIG_OVERLAY: usize = 1;
pub(super) const FOCUS_ELICITATION: usize = 2;

pub struct UiState {
    pub tool_call_statuses: ToolCallStatuses,
    pub(crate) grid_loader: Spinner,
    pub(crate) conversation: ConversationBuffer,
    pub(crate) prompt_composer: PromptComposer,
    pub(crate) agent_name: String,
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
    pub git_diff_mode: GitDiffMode,
    pub exit_requested: bool,
    pub(super) focus: FocusRing,
}

impl UiState {
    pub fn new(
        agent_name: String,
        config_options: &[SessionConfigOption],
        auth_methods: Vec<acp::AuthMethod>,
        working_dir: PathBuf,
    ) -> Self {
        let keybindings = Keybindings::default();
        Self {
            tool_call_statuses: ToolCallStatuses::new(),
            grid_loader: Spinner::default(),
            conversation: ConversationBuffer::new(),
            prompt_composer: PromptComposer::new(keybindings.clone()),
            agent_name,
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
            git_diff_mode: GitDiffMode::new(working_dir),
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

    pub(crate) fn refresh_caches(&mut self, context: &ViewContext) {
        let progress = self.tool_call_statuses.progress();
        self.progress_indicator
            .update(progress.completed_top_level, progress.total_top_level);

        if matches!(self.screen_mode, ScreenMode::GitDiff) {
            self.git_diff_mode.refresh_caches(context);
        }
    }

    pub fn prepare_for_render(&mut self, context: &ViewContext) {
        self.refresh_caches(context);

        if let Some(ref mut overlay) = self.config_overlay {
            let height = (context.size.height.saturating_sub(1)) as usize;
            if height >= 3 {
                overlay.update_child_viewport(height.saturating_sub(4));
            }
        }

        self.plan_tracker.cached_visible_entries();
    }

    #[allow(dead_code)]
    pub(crate) fn on_authenticate_started(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.on_authenticate_started(method_id);
        }
    }

    pub fn drain_completed(&mut self) -> (Vec<SegmentContent>, Vec<String>) {
        self.conversation
            .drain_completed(&self.tool_call_statuses)
    }

    pub fn remove_tools(&mut self, tool_ids: &[String]) {
        for id in tool_ids {
            self.tool_call_statuses.remove_tool(id);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_state_new_initializes_defaults() {
        let state = UiState::new("test-agent".to_string(), &[], vec![], PathBuf::from("."));

        assert!(!state.waiting_for_response);
        assert_eq!(state.context_usage_pct, None);
        assert!(state.config_overlay.is_none());
        assert!(state.elicitation_form.is_none());
        assert!(state.server_statuses.is_empty());
    }
}
