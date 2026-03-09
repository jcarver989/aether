use super::{AppAction, UiState};
use crate::components::command_picker::CommandEntry;
use crate::components::elicitation_form::ElicitationForm;
use crate::components::progress_indicator::ProgressIndicator;
use crate::tui::Action;
use crate::tui::RenderContext;
use acp_utils::notifications::{
    CONTEXT_CLEARED_METHOD, CONTEXT_USAGE_METHOD, ContextUsageParams, ElicitationParams,
    ElicitationResponse, McpNotification, SUB_AGENT_PROGRESS_METHOD, SubAgentProgressParams,
};
use agent_client_protocol::{self as acp, ExtNotification, SessionUpdate};
use std::time::Instant;
use tokio::sync::oneshot;

impl UiState {
    pub(crate) fn on_session_update(&mut self, update: SessionUpdate) -> Vec<Action<AppAction>> {
        let was_loading = self.grid_loader.visible;
        let mut should_render = was_loading;
        self.grid_loader.visible = false;

        match update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let acp::ContentBlock::Text(text_content) = chunk.content {
                    self.conversation.append_text_chunk(&text_content.text);
                    should_render = true;
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let acp::ContentBlock::Text(text_content) = chunk.content {
                    self.conversation.append_thought_chunk(&text_content.text);
                    should_render = true;
                }
            }
            SessionUpdate::ToolCall(tool_call) => {
                self.conversation.close_thought_block();
                self.tool_call_statuses.on_tool_call(&tool_call);
                self.conversation
                    .ensure_tool_segment(&tool_call.tool_call_id.0);
                self.conversation
                    .invalidate_tool_segment(&tool_call.tool_call_id.0);
                should_render = true;
            }
            SessionUpdate::ToolCallUpdate(update) => {
                self.conversation.close_thought_block();
                self.tool_call_statuses.on_tool_call_update(&update);
                if self.tool_call_statuses.has_tool(&update.tool_call_id.0) {
                    self.conversation
                        .ensure_tool_segment(&update.tool_call_id.0);
                    self.conversation
                        .invalidate_tool_segment(&update.tool_call_id.0);
                }
                should_render = true;
            }
            SessionUpdate::AvailableCommandsUpdate(update) => {
                let commands = update
                    .available_commands
                    .into_iter()
                    .map(|cmd| {
                        let hint = match cmd.input {
                            Some(acp::AvailableCommandInput::Unstructured(ref input)) => {
                                Some(input.hint.clone())
                            }
                            _ => None,
                        };
                        CommandEntry {
                            name: cmd.name,
                            description: cmd.description,
                            has_input: cmd.input.is_some(),
                            hint,
                            builtin: false,
                        }
                    })
                    .collect();
                self.prompt_composer.set_available_commands(commands);
            }
            SessionUpdate::ConfigOptionUpdate(update) => {
                self.conversation.close_thought_block();
                self.update_config_options(&update.config_options);
                if let Some(ref mut overlay) = self.config_overlay {
                    overlay.update_config_options(&update.config_options);
                }
                should_render = true;
            }
            SessionUpdate::Plan(plan) => {
                self.plan_tracker.replace(plan.entries, Instant::now());
                should_render = true;
            }
            _ => {
                self.conversation.close_thought_block();
            }
        }

        if should_render {
            vec![Action::Render]
        } else {
            vec![]
        }
    }

    pub(crate) fn on_prompt_done(&mut self, context: &RenderContext) -> Vec<Action<AppAction>> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        self.conversation.close_thought_block();

        let (scrollback_lines, completed_tool_ids) = self
            .conversation
            .flush_completed(&self.tool_call_statuses, context);

        for id in completed_tool_ids {
            self.tool_call_statuses.remove_tool(&id);
        }

        let mut effects = Vec::new();
        if !scrollback_lines.is_empty() {
            effects.push(Action::Custom(AppAction::PushScrollback(scrollback_lines)));
        }
        effects.push(Action::Render);
        effects
    }

    pub(crate) fn on_tick(&mut self) -> Vec<Action<AppAction>> {
        let now = Instant::now();
        let mut should_render = false;

        self.grid_loader.on_tick();
        should_render |= self.grid_loader.visible;

        self.tool_call_statuses.on_tick(now);
        should_render |= self.tool_call_statuses.progress().running_any;

        self.plan_tracker.on_tick(now);
        should_render |= self
            .plan_tracker
            .visible_entries(now, self.plan_tracker.grace_period)
            .iter()
            .any(|entry| matches!(entry.status, acp::PlanEntryStatus::Completed));

        self.progress_indicator.on_tick();
        should_render |= self.progress_indicator.total > 0
            && self.progress_indicator.completed < self.progress_indicator.total;

        if should_render {
            vec![Action::Render]
        } else {
            vec![]
        }
    }

    pub(crate) fn on_elicitation_request(
        &mut self,
        params: ElicitationParams,
        response_tx: oneshot::Sender<ElicitationResponse>,
    ) -> Vec<Action<AppAction>> {
        self.config_overlay = None;
        self.elicitation_form = Some(ElicitationForm::from_params(params, response_tx));
        vec![Action::Render]
    }

    pub(crate) fn on_ext_notification(
        &mut self,
        notification: ExtNotification,
    ) -> Vec<Action<AppAction>> {
        match notification.method.as_ref() {
            CONTEXT_CLEARED_METHOD => {
                self.reset_after_context_cleared();
                vec![Action::Render]
            }
            CONTEXT_USAGE_METHOD => {
                if let Ok(params) =
                    serde_json::from_str::<ContextUsageParams>(notification.params.get())
                {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        self.context_usage_pct = params.usage_ratio.map(|usage_ratio| {
                            ((1.0 - usage_ratio) * 100.0).clamp(0.0, 100.0).round() as u8
                        });
                    }
                    vec![Action::Render]
                } else {
                    vec![]
                }
            }
            SUB_AGENT_PROGRESS_METHOD => {
                if let Ok(progress) =
                    serde_json::from_str::<SubAgentProgressParams>(notification.params.get())
                {
                    self.tool_call_statuses.on_sub_agent_progress(&progress);
                    self.conversation
                        .invalidate_tool_segment(&progress.parent_tool_id);
                    vec![Action::Render]
                } else {
                    vec![]
                }
            }
            _ => {
                if let Ok(McpNotification::ServerStatus { servers }) =
                    McpNotification::try_from(&notification)
                {
                    self.server_statuses.clone_from(&servers);
                    if let Some(ref mut overlay) = self.config_overlay {
                        overlay.update_server_statuses(servers);
                    }
                    vec![Action::Render]
                } else {
                    vec![]
                }
            }
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

    pub(crate) fn on_prompt_error(&mut self, error: &acp::Error) -> Vec<Action<AppAction>> {
        tracing::error!("Prompt error: {error}");
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        vec![Action::Render]
    }

    pub(crate) fn on_authenticate_complete(&mut self, method_id: &str) -> Vec<Action<AppAction>> {
        self.auth_methods
            .retain(|method| method.id.0.as_ref() != method_id);
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.remove_auth_method(method_id);
        }
        vec![Action::Render]
    }

    pub(crate) fn on_authenticate_failed(
        &mut self,
        method_id: &str,
        error: &str,
    ) -> Vec<Action<AppAction>> {
        tracing::warn!("Provider auth failed for {method_id}: {error}");
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.on_authenticate_failed(method_id);
        }
        vec![Action::Render]
    }

    pub(crate) fn on_connection_closed(&mut self) -> Vec<Action<AppAction>> {
        vec![Action::Exit]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::Action;

    #[test]
    fn on_prompt_error_clears_waiting_state_and_requests_render() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        state.waiting_for_response = true;
        state.grid_loader.visible = true;

        let error = acp::Error::internal_error();
        let actions = state.on_prompt_error(&error);

        assert!(!state.waiting_for_response);
        assert!(!state.grid_loader.visible);
        assert!(matches!(actions.as_slice(), [Action::Render]));
    }

    #[test]
    fn on_authenticate_complete_removes_method_and_requests_render() {
        let mut state = UiState::new(
            "test-agent".to_string(),
            &[],
            vec![acp::AuthMethod::new("anthropic", "Anthropic")],
        );

        let actions = state.on_authenticate_complete("anthropic");

        assert!(state.auth_methods.is_empty());
        assert!(matches!(actions.as_slice(), [Action::Render]));
    }

    #[test]
    fn on_authenticate_failed_requests_render() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);

        let actions = state.on_authenticate_failed("anthropic", "bad token");

        assert!(matches!(actions.as_slice(), [Action::Render]));
    }

    #[test]
    fn on_connection_closed_requests_exit() {
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);

        let actions = state.on_connection_closed();

        assert!(matches!(actions.as_slice(), [Action::Exit]));
    }
}
