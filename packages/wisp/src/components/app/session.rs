use super::{AppEffect, UiState};
use crate::components::command_picker::CommandEntry;
use crate::components::elicitation_form::ElicitationForm;
use crate::components::progress_indicator::ProgressIndicator;
use crate::tui::{RenderContext, TickableComponent};
use acp_utils::notifications::{
    CONTEXT_CLEARED_METHOD, CONTEXT_USAGE_METHOD, ContextUsageParams, ElicitationParams,
    ElicitationResponse, McpNotification, SUB_AGENT_PROGRESS_METHOD, SubAgentProgressParams,
};
use agent_client_protocol::{self as acp, ExtNotification, SessionUpdate};
use std::time::Instant;
use tokio::sync::oneshot;

impl UiState {
    pub(crate) fn on_session_update(&mut self, update: SessionUpdate) -> Vec<AppEffect> {
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
                should_render = true;
            }
            SessionUpdate::ToolCallUpdate(update) => {
                self.conversation.close_thought_block();
                self.tool_call_statuses.on_tool_call_update(&update);
                if self.tool_call_statuses.has_tool(&update.tool_call_id.0) {
                    self.conversation
                        .ensure_tool_segment(&update.tool_call_id.0);
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
            vec![AppEffect::Render]
        } else {
            vec![]
        }
    }

    pub(crate) fn on_prompt_done(&mut self, context: &RenderContext) -> Vec<AppEffect> {
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
            effects.push(AppEffect::PushScrollback(scrollback_lines));
        }
        effects.push(AppEffect::Render);
        effects
    }

    pub(crate) fn on_tick(&mut self) -> Vec<AppEffect> {
        let now = Instant::now();
        self.grid_loader.on_tick(now);
        self.tool_call_statuses.on_tick(now);
        self.plan_tracker.on_tick(now);
        self.progress_indicator.on_tick(now);
        vec![AppEffect::Render]
    }

    pub(crate) fn on_elicitation_request(
        &mut self,
        params: ElicitationParams,
        response_tx: oneshot::Sender<ElicitationResponse>,
    ) -> Vec<AppEffect> {
        self.config_overlay = None;
        self.elicitation_form = Some(ElicitationForm::from_params(params, response_tx));
        vec![AppEffect::Render]
    }

    pub(crate) fn on_ext_notification(&mut self, notification: ExtNotification) -> Vec<AppEffect> {
        match notification.method.as_ref() {
            CONTEXT_CLEARED_METHOD => {
                self.reset_after_context_cleared();
                vec![AppEffect::Render]
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
                    vec![AppEffect::Render]
                } else {
                    vec![]
                }
            }
            SUB_AGENT_PROGRESS_METHOD => {
                if let Ok(progress) =
                    serde_json::from_str::<SubAgentProgressParams>(notification.params.get())
                {
                    self.tool_call_statuses.on_sub_agent_progress(&progress);
                    vec![AppEffect::Render]
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
                    vec![AppEffect::Render]
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

    pub(crate) fn on_prompt_error(&mut self) -> Vec<AppEffect> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        vec![AppEffect::Render]
    }

    pub(crate) fn on_authenticate_complete(&mut self, method_id: &str) {
        self.auth_methods
            .retain(|method| method.id.0.as_ref() != method_id);
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.remove_auth_method(method_id);
        }
    }

    pub(crate) fn on_authenticate_failed(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.on_authenticate_failed(method_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::app::{App, AppAction};
    use crate::components::conversation_window::SegmentContent;
    use crate::tui::CursorComponent;
    use acp_utils::notifications::{CONTEXT_USAGE_METHOD, McpServerStatus, McpServerStatusEntry};
    use agent_client_protocol::SessionConfigOptionCategory;
    use std::sync::Arc;

    #[test]
    fn prompt_done_keeps_running_tool_segment() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);

        let tool_call = acp::ToolCall::new("tool-1", "Read file");
        screen.state.tool_call_statuses.on_tool_call(&tool_call);
        screen.state.conversation.ensure_tool_segment("tool-1");

        let effects = screen.dispatch(AppAction::PromptDone, &RenderContext::new((120, 40)));

        assert!(matches!(effects.as_slice(), [AppEffect::Render]));
        let segments: Vec<_> = screen.state.conversation.segments().collect();
        assert!(matches!(segments[..], [SegmentContent::ToolCall(id)] if id == "tool-1"));
    }

    #[test]
    fn prompt_done_flush_respects_custom_theme() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen
            .state
            .conversation
            .append_thought_chunk("theme should be preserved");

        let context = RenderContext::new((120, 40));
        let effects = screen.dispatch(AppAction::PromptDone, &context);
        assert!(matches!(effects.last(), Some(AppEffect::Render)));
    }

    #[test]
    fn streaming_chunks_keep_waiting_for_response() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen.state.waiting_for_response = true;

        screen.dispatch(
            AppAction::SessionUpdate(SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                acp::ContentBlock::Text(acp::TextContent::new("hello")),
            ))),
            &RenderContext::new((120, 40)),
        );

        assert!(screen.state.waiting_for_response);
    }

    #[test]
    fn sub_agent_progress_notification_triggers_render() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let json = r#"{"parent_tool_id":"p1","task_id":"t1","agent_name":"explorer","event":{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}}"#;
        let raw = serde_json::value::to_raw_value(
            &serde_json::from_str::<serde_json::Value>(json).unwrap(),
        )
        .unwrap();
        let notification =
            acp::ExtNotification::new("_aether/sub_agent_progress", std::sync::Arc::from(raw));

        let effects = app.dispatch(
            AppAction::ExtNotification(notification),
            &RenderContext::new((120, 40)),
        );
        assert!(matches!(effects.as_slice(), [AppEffect::Render]));
    }

    #[test]
    fn invalid_sub_agent_progress_json_silently_ignored() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let raw = serde_json::value::to_raw_value(&serde_json::json!({"bad": "data"})).unwrap();
        let notification =
            acp::ExtNotification::new("_aether/sub_agent_progress", std::sync::Arc::from(raw));

        let effects = app.dispatch(
            AppAction::ExtNotification(notification),
            &RenderContext::new((120, 40)),
        );
        assert!(effects.is_empty());
    }

    #[test]
    fn context_usage_notification_updates_percent_left() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let raw = serde_json::value::to_raw_value(&serde_json::json!({
            "usage_ratio": 0.75,
            "tokens_used": 150_000,
            "context_limit": 200_000
        }))
        .unwrap();
        let notification = acp::ExtNotification::new(CONTEXT_USAGE_METHOD, Arc::from(raw));

        let effects = app.dispatch(
            AppAction::ExtNotification(notification),
            &RenderContext::new((120, 40)),
        );

        assert!(matches!(effects.as_slice(), [AppEffect::Render]));
        assert_eq!(app.state.context_usage_pct, Some(25));
    }

    #[test]
    fn context_usage_notification_with_unknown_limit_clears_meter() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        app.state.context_usage_pct = Some(33);

        let raw = serde_json::value::to_raw_value(&serde_json::json!({
            "usage_ratio": null,
            "tokens_used": 0,
            "context_limit": null
        }))
        .unwrap();
        let notification = acp::ExtNotification::new(CONTEXT_USAGE_METHOD, Arc::from(raw));

        let effects = app.dispatch(
            AppAction::ExtNotification(notification),
            &RenderContext::new((120, 40)),
        );

        assert!(matches!(effects.as_slice(), [AppEffect::Render]));
        assert_eq!(app.state.context_usage_pct, None);
    }

    #[test]
    fn context_cleared_notification_resets_conversation_and_tool_state() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        app.state.waiting_for_response = true;
        app.state.grid_loader.visible = true;
        app.state.context_usage_pct = Some(25);
        app.state
            .conversation
            .set_segments(vec![SegmentContent::Text("hello".to_string())]);
        app.state
            .tool_call_statuses
            .on_tool_call(&acp::ToolCall::new("tool-1", "Read file"));

        let raw = serde_json::value::to_raw_value(&serde_json::json!({})).unwrap();
        let notification = acp::ExtNotification::new(CONTEXT_CLEARED_METHOD, Arc::from(raw));

        let effects = app.dispatch(
            AppAction::ExtNotification(notification),
            &RenderContext::new((120, 40)),
        );

        assert!(matches!(effects.as_slice(), [AppEffect::Render]));
        assert!(!app.state.waiting_for_response);
        assert!(!app.state.grid_loader.visible);
        assert_eq!(app.state.context_usage_pct, None);
        assert_eq!(app.state.conversation.segments().len(), 0);
        assert_eq!(app.state.tool_call_statuses.progress().total_top_level, 0);
    }

    #[test]
    fn on_tick_requests_render_while_completed_entries_waiting_to_expire() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        app.state.plan_tracker.replace(
            vec![acp::PlanEntry::new(
                "1",
                acp::PlanEntryPriority::Medium,
                acp::PlanEntryStatus::Completed,
            )],
            Instant::now(),
        );

        let effects = app.dispatch(AppAction::Tick, &RenderContext::new((120, 40)));
        assert!(matches!(effects.as_slice(), [AppEffect::Render]));
    }

    #[test]
    fn on_tick_always_emits_render() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let effects = app.dispatch(AppAction::Tick, &RenderContext::new((120, 40)));
        assert!(matches!(effects.as_slice(), [AppEffect::Render]));
    }

    #[test]
    fn config_option_update_refreshes_mode_display() {
        let initial = vec![
            acp::SessionConfigOption::select(
                "mode",
                "Mode",
                "planner",
                vec![
                    acp::SessionConfigSelectOption::new("planner", "Planner"),
                    acp::SessionConfigSelectOption::new("coder", "Coder"),
                ],
            )
            .category(SessionConfigOptionCategory::Mode),
        ];
        let updated = vec![
            acp::SessionConfigOption::select(
                "mode",
                "Mode",
                "coder",
                vec![
                    acp::SessionConfigSelectOption::new("planner", "Planner"),
                    acp::SessionConfigSelectOption::new("coder", "Coder"),
                ],
            )
            .category(SessionConfigOptionCategory::Mode),
        ];
        let mut app = App::new("test-agent".to_string(), &initial, vec![]);

        let effects = app.dispatch(
            AppAction::SessionUpdate(SessionUpdate::ConfigOptionUpdate(
                acp::ConfigOptionUpdate::new(updated),
            )),
            &RenderContext::new((120, 40)),
        );

        assert!(matches!(effects.as_slice(), [AppEffect::Render]));
        let output = app.render(&RenderContext::new((120, 40)));
        assert!(
            output
                .lines
                .iter()
                .any(|line| line.plain_text().contains("Coder"))
        );
    }

    #[test]
    fn available_commands_update_is_forwarded_to_prompt_composer() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);

        let effects = app.dispatch(
            AppAction::SessionUpdate(SessionUpdate::AvailableCommandsUpdate(
                acp::AvailableCommandsUpdate::new(vec![acp::AvailableCommand::new(
                    "search",
                    "Search code",
                )]),
            )),
            &RenderContext::new((120, 40)),
        );

        assert!(effects.is_empty());
        assert_eq!(app.state.available_commands().len(), 1);
        assert_eq!(app.state.available_commands()[0].name, "search");
    }

    #[test]
    fn server_status_notification_updates_overlay_state() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        app.state.open_config_overlay();
        let notification =
            acp::ExtNotification::from(acp_utils::notifications::McpNotification::ServerStatus {
                servers: vec![McpServerStatusEntry {
                    name: "docs".to_string(),
                    status: McpServerStatus::Connected { tool_count: 0 },
                }],
            });

        let effects = app.dispatch(
            AppAction::ExtNotification(notification),
            &RenderContext::new((120, 40)),
        );

        assert!(matches!(effects.as_slice(), [AppEffect::Render]));
        assert_eq!(app.state.server_statuses.len(), 1);
        assert!(matches!(
            app.state.server_statuses[0],
            McpServerStatusEntry {
                ref name,
                status: McpServerStatus::Connected { .. }
            } if name == "docs"
        ));
    }
}
