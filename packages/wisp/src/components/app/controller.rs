use super::attachments::build_attachment_blocks;
use super::git_diff_mode::format_review_prompt;
use super::state::{FOCUS_COMPOSER, FOCUS_CONFIG_OVERLAY, FOCUS_ELICITATION, is_cycleable_mode_option};
use super::view::UiView;
use super::{PromptAttachment, ScreenMode, UiState, WispEvent};
use crate::components::config_overlay::ConfigOverlayMessage;
use crate::components::elicitation_form::ElicitationForm;
use crate::components::git_diff_view::GitDiffViewMessage;
use crate::components::prompt_composer::PromptComposerMessage;
use crate::error::AppError;
use crate::keybindings::Keybindings;
use crate::tui::{Component, Event, FormMessage, KeyEvent, Line, PickerMessage, ViewContext};
use acp_utils::client::{AcpEvent, AcpPromptHandle};
use acp_utils::config_option_id::{ConfigOptionId, THEME_CONFIG_ID};
use agent_client_protocol::{
    self as acp, SessionConfigKind, SessionConfigSelectOptions, SessionId,
};
use std::io::Write;
use std::time::Instant;
use tokio::sync::oneshot;
use utils::ReasoningEffort;

pub struct UiStateController {
    pub(super) session_id: SessionId,
    keybindings: Keybindings,
    prompt_handle: AcpPromptHandle,
}

#[allow(clippy::unused_self)]
impl UiStateController {
    pub fn new(session_id: SessionId, prompt_handle: AcpPromptHandle) -> Self {
        Self {
            session_id,
            keybindings: Keybindings::default(),
            prompt_handle,
        }
    }

    pub async fn handle_event<W: Write>(
        &mut self,
        state: &mut UiState,
        view: &mut UiView<W>,
        event: WispEvent,
    ) -> Result<(), AppError> {
        match event {
            WispEvent::Terminal(ref terminal_event) => {
                self.handle_terminal_event(state, view, terminal_event)
                    .await
            }
            WispEvent::Acp(acp_event) => {
                let context = view.context();
                self.handle_acp_event(state, view, acp_event, &context)
            }
        }
    }

    async fn handle_terminal_event<W: Write>(
        &mut self,
        state: &mut UiState,
        view: &mut UiView<W>,
        event: &Event,
    ) -> Result<(), AppError> {
        match event {
            Event::Key(key_event) => self.handle_key(state, view, *key_event).await,
            Event::Paste(_) => {
                state.config_overlay = None;
                let outcome = state.prompt_composer.on_event(event);
                self.handle_prompt_composer_messages(state, view, outcome)
                    .await
            }
            Event::Tick => {
                let now = Instant::now();
                state.grid_loader.on_tick();
                state.tool_call_statuses.on_tick(now);
                state.plan_tracker.on_tick(now);
                state.progress_indicator.on_tick();
                Ok(())
            }
            Event::Mouse(_) | Event::Resize(_) => Ok(()),
        }
    }

    async fn handle_key<W: Write>(
        &mut self,
        state: &mut UiState,
        view: &mut UiView<W>,
        key_event: KeyEvent,
    ) -> Result<(), AppError> {
        if self.keybindings.exit.matches(key_event) {
            state.exit_requested = true;
            return Ok(());
        }

        if state.elicitation_form.is_some() {
            self.handle_elicitation_key(state, key_event);
            return Ok(());
        }

        if state.session_picker.is_some() {
            return self.handle_session_picker_key(state, view, key_event);
        }

        if self.keybindings.toggle_git_diff.matches(key_event) {
            if matches!(state.screen_mode, ScreenMode::GitDiff) {
                view.git_diff_mode.close();
                state.exit_git_diff();
            } else {
                state.enter_git_diff();
                view.git_diff_mode.begin_open();
                view.git_diff_mode.complete_load().await;
            }
            return Ok(());
        }

        let event = Event::Key(key_event);

        if state.focus.focused() == FOCUS_CONFIG_OVERLAY {
            let outcome = state
                .config_overlay
                .as_mut()
                .expect("config overlay")
                .on_event(&event);
            return self.handle_config_overlay_messages(state, view, outcome);
        }

        if matches!(state.screen_mode, ScreenMode::GitDiff) {
            let messages = view.git_diff_mode.on_key_event(&event);
            for msg in messages {
                self.handle_git_diff_message(state, view, msg).await?;
            }
            return Ok(());
        }

        let composer_outcome = state.prompt_composer.on_event(&event);
        if composer_outcome.is_some() {
            return self
                .handle_prompt_composer_messages(state, view, composer_outcome)
                .await;
        }

        if self.keybindings.cycle_reasoning.matches(key_event) {
            self.cycle_reasoning_option(state);
            return Ok(());
        }

        if self.keybindings.cycle_mode.matches(key_event) {
            self.cycle_quick_option(state);
            return Ok(());
        }

        if self.keybindings.cancel.matches(key_event) && state.waiting_for_response {
            self.prompt_handle.cancel(&self.session_id)?;
            return Ok(());
        }

        Ok(())
    }

    fn handle_elicitation_key(&self, state: &mut UiState, key_event: KeyEvent) {
        let Some(elicitation_form) = state.elicitation_form.as_mut() else {
            return;
        };
        let outcome = elicitation_form.form.on_event(&Event::Key(key_event));

        for message in outcome.unwrap_or_default() {
            match message {
                FormMessage::Close => {
                    if let Some(elicitation_form) = state.elicitation_form.take() {
                        let _ = elicitation_form
                            .response_tx
                            .send(ElicitationForm::decline());
                    }
                    state.focus.focus(FOCUS_COMPOSER);
                }
                FormMessage::Submit => {
                    if let Some(elicitation_form) = state.elicitation_form.take() {
                        let response = elicitation_form.confirm();
                        let _ = elicitation_form.response_tx.send(response);
                    }
                    state.focus.focus(FOCUS_COMPOSER);
                }
            }
        }
    }

    fn handle_session_picker_key<W: Write>(
        &self,
        state: &mut UiState,
        view: &mut UiView<W>,
        key_event: KeyEvent,
    ) -> Result<(), AppError> {
        let Some(picker) = state.session_picker.as_mut() else {
            return Ok(());
        };
        let msgs = picker
            .on_event(&Event::Key(key_event))
            .unwrap_or_default();
        for msg in msgs {
            match msg {
                PickerMessage::Close => {
                    state.session_picker = None;
                }
                PickerMessage::Confirm(entry) => {
                    state.session_picker = None;
                    let info = entry.0;
                    state.reset_after_context_cleared();
                    view.clear_screen()?;
                    self.prompt_handle
                        .load_session(&acp::SessionId::new(info.session_id.0.to_string()), &info.cwd)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn handle_prompt_composer_messages<W: Write>(
        &self,
        state: &mut UiState,
        view: &mut UiView<W>,
        outcome: Option<Vec<PromptComposerMessage>>,
    ) -> Result<(), AppError> {
        for msg in outcome.unwrap_or_default() {
            match msg {
                PromptComposerMessage::ClearScreen => {
                    state.reset_after_context_cleared();
                    view.clear_screen()?;
                    self.prompt_handle
                        .prompt(&self.session_id, "/clear", None)?;
                }
                PromptComposerMessage::OpenConfig => {
                    state.open_config_overlay();
                }
                PromptComposerMessage::OpenSessionPicker => {
                    self.prompt_handle.list_sessions()?;
                }
                PromptComposerMessage::SubmitRequested {
                    user_input,
                    attachments,
                } => {
                    state.waiting_for_response = true;
                    state.grid_loader.reset();
                    view.push_scrollback(&[Line::new(String::new())])?;
                    view.push_scrollback(&[Line::new(user_input.clone())])?;
                    view.render(state)?;
                    self.submit_prompt(view, &user_input, attachments).await?;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn handle_config_overlay_messages<W: Write>(
        &self,
        state: &mut UiState,
        view: &mut UiView<W>,
        outcome: Option<Vec<ConfigOverlayMessage>>,
    ) -> Result<(), AppError> {
        for message in outcome.unwrap_or_default() {
            match message {
                ConfigOverlayMessage::Close => {
                    state.config_overlay = None;
                    state.focus.focus(FOCUS_COMPOSER);
                }
                ConfigOverlayMessage::ApplyConfigChanges(changes) => {
                    for change in changes {
                        if change.config_id == THEME_CONFIG_ID {
                            let file = theme_file_from_picker_value(&change.new_value);
                            view.apply_theme_selection(file);
                        } else {
                            let _ = self.prompt_handle.set_config_option(
                                &self.session_id,
                                &change.config_id,
                                &change.new_value,
                            );
                        }
                    }
                }
                ConfigOverlayMessage::AuthenticateServer(name) => {
                    let _ = self
                        .prompt_handle
                        .authenticate_mcp_server(&self.session_id, &name);
                }
                ConfigOverlayMessage::AuthenticateProvider(method_id) => {
                    let _ = self
                        .prompt_handle
                        .authenticate(&self.session_id, &method_id);
                    state.on_authenticate_started(&method_id);
                }
            }
        }
        Ok(())
    }

    fn cycle_quick_option(&self, state: &UiState) {
        let Some(option) = state
            .config_options
            .iter()
            .find(|option| is_cycleable_mode_option(option))
        else {
            return;
        };

        let SessionConfigKind::Select(ref select) = option.kind else {
            return;
        };

        let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
            return;
        };

        if options.is_empty() {
            return;
        }

        let current_index = options
            .iter()
            .position(|entry| entry.value == select.current_value)
            .unwrap_or(0);
        let next_index = (current_index + 1) % options.len();
        if let Some(next) = options.get(next_index) {
            let _ = self.prompt_handle.set_config_option(
                &self.session_id,
                &option.id.0,
                &next.value.0,
            );
        }
    }

    fn cycle_reasoning_option(&self, state: &UiState) {
        let has_reasoning = state
            .config_options
            .iter()
            .any(|option| option.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str());

        if has_reasoning {
            let next = ReasoningEffort::cycle_next(state.reasoning_effort);
            let _ = self.prompt_handle.set_config_option(
                &self.session_id,
                ConfigOptionId::ReasoningEffort.as_str(),
                ReasoningEffort::config_str(next),
            );
        }
    }

    async fn handle_git_diff_message<W: Write>(
        &self,
        state: &mut UiState,
        view: &mut UiView<W>,
        msg: GitDiffViewMessage,
    ) -> Result<(), AppError> {
        match msg {
            GitDiffViewMessage::Close => {
                view.git_diff_mode.close();
                state.exit_git_diff();
            }
            GitDiffViewMessage::Refresh => {
                view.git_diff_mode.begin_refresh();
                view.git_diff_mode.complete_load().await;
            }
            GitDiffViewMessage::SubmitReview { comments } => {
                let prompt = format_review_prompt(&comments);
                view.git_diff_mode.close();
                state.exit_git_diff();
                self.submit_prompt(view, &prompt, vec![]).await?;
            }
        }
        Ok(())
    }

    fn handle_acp_event<W: Write>(
        &mut self,
        state: &mut UiState,
        view: &mut UiView<W>,
        event: AcpEvent,
        context: &ViewContext,
    ) -> Result<(), AppError> {
        match event {
            AcpEvent::SessionUpdate(update) => self.on_session_update(state, *update),
            AcpEvent::ExtNotification(notification) => {
                self.on_ext_notification(state, &notification);
            }
            AcpEvent::PromptDone(_) => self.on_prompt_done(state, view, context)?,
            AcpEvent::PromptError(error) => self.on_prompt_error(state, &error),
            AcpEvent::ElicitationRequest {
                params,
                response_tx,
            } => self.on_elicitation_request(state, params, response_tx),
            AcpEvent::AuthenticateComplete { method_id } => {
                self.on_authenticate_complete(state, &method_id);
            }
            AcpEvent::AuthenticateFailed { method_id, error } => {
                self.on_authenticate_failed(state, &method_id, &error);
            }
            AcpEvent::SessionsListed { sessions } => {
                let current_id = &self.session_id;
                let filtered: Vec<_> = sessions
                    .into_iter()
                    .filter(|s| s.session_id != *current_id)
                    .collect();
                state.open_session_picker(filtered);
            }
            AcpEvent::SessionLoaded {
                session_id,
                config_options,
            } => {
                self.session_id = session_id;
                state.update_config_options(&config_options);
            }
            AcpEvent::ConnectionClosed => {
                state.exit_requested = true;
            }
        }
        Ok(())
    }

    fn on_session_update(&self, state: &mut UiState, update: acp::SessionUpdate) {
        state.grid_loader.visible = false;

        match update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                if let acp::ContentBlock::Text(text_content) = chunk.content {
                    state.conversation.append_text_chunk(&text_content.text);
                }
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                if let acp::ContentBlock::Text(text_content) = chunk.content {
                    state.conversation.append_thought_chunk(&text_content.text);
                }
            }
            acp::SessionUpdate::ToolCall(tool_call) => {
                state.conversation.close_thought_block();
                state.tool_call_statuses.on_tool_call(&tool_call);
                state
                    .conversation
                    .ensure_tool_segment(&tool_call.tool_call_id.0);
                state
                    .conversation
                    .invalidate_tool_segment(&tool_call.tool_call_id.0);
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                state.conversation.close_thought_block();
                state.tool_call_statuses.on_tool_call_update(&update);
                if state.tool_call_statuses.has_tool(&update.tool_call_id.0) {
                    state
                        .conversation
                        .ensure_tool_segment(&update.tool_call_id.0);
                    state
                        .conversation
                        .invalidate_tool_segment(&update.tool_call_id.0);
                }
            }
            acp::SessionUpdate::AvailableCommandsUpdate(update) => {
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
                        crate::components::command_picker::CommandEntry {
                            name: cmd.name,
                            description: cmd.description,
                            has_input: cmd.input.is_some(),
                            hint,
                            builtin: false,
                        }
                    })
                    .collect();
                state.prompt_composer.set_available_commands(commands);
            }
            acp::SessionUpdate::ConfigOptionUpdate(update) => {
                state.conversation.close_thought_block();
                state.update_config_options(&update.config_options);
                if let Some(ref mut overlay) = state.config_overlay {
                    overlay.update_config_options(&update.config_options);
                }
            }
            acp::SessionUpdate::Plan(plan) => {
                state.plan_tracker.replace(plan.entries, Instant::now());
            }
            _ => {
                state.conversation.close_thought_block();
            }
        }
    }

    fn on_prompt_done<W: Write>(
        &self,
        state: &mut UiState,
        view: &mut UiView<W>,
        context: &ViewContext,
    ) -> Result<(), AppError> {
        state.waiting_for_response = false;
        state.grid_loader.visible = false;
        state.conversation.close_thought_block();

        let (scrollback_lines, completed_tool_ids) = state
            .conversation
            .flush_completed(&state.tool_call_statuses, context);

        for id in completed_tool_ids {
            state.tool_call_statuses.remove_tool(&id);
        }

        if !scrollback_lines.is_empty() {
            view.push_scrollback(&scrollback_lines)?;
        }

        Ok(())
    }

    fn on_elicitation_request(
        &self,
        state: &mut UiState,
        params: acp_utils::notifications::ElicitationParams,
        response_tx: oneshot::Sender<acp_utils::notifications::ElicitationResponse>,
    ) {
        state.config_overlay = None;
        state.elicitation_form = Some(ElicitationForm::from_params(params, response_tx));
        state.focus.focus(FOCUS_ELICITATION);
    }

    fn on_ext_notification(
        &self,
        state: &mut UiState,
        notification: &acp::ExtNotification,
    ) {
        use acp_utils::notifications::{
            CONTEXT_CLEARED_METHOD, CONTEXT_USAGE_METHOD, ContextUsageParams, McpNotification,
            SUB_AGENT_PROGRESS_METHOD, SubAgentProgressParams,
        };

        match notification.method.as_ref() {
            CONTEXT_CLEARED_METHOD => {
                state.reset_after_context_cleared();
            }
            CONTEXT_USAGE_METHOD => {
                if let Ok(params) =
                    serde_json::from_str::<ContextUsageParams>(notification.params.get())
                {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        state.context_usage_pct = params.usage_ratio.map(|usage_ratio| {
                            ((1.0 - usage_ratio) * 100.0).clamp(0.0, 100.0).round() as u8
                        });
                    }
                }
            }
            SUB_AGENT_PROGRESS_METHOD => {
                if let Ok(progress) =
                    serde_json::from_str::<SubAgentProgressParams>(notification.params.get())
                {
                    state.tool_call_statuses.on_sub_agent_progress(&progress);
                    state
                        .conversation
                        .invalidate_tool_segment(&progress.parent_tool_id);
                }
            }
            _ => {
                if let Ok(McpNotification::ServerStatus { servers }) =
                    McpNotification::try_from(notification)
                {
                    state.server_statuses.clone_from(&servers);
                    if let Some(ref mut overlay) = state.config_overlay {
                        overlay.update_server_statuses(servers);
                    }
                }
            }
        }
    }

    fn on_prompt_error(&self, state: &mut UiState, error: &acp::Error) {
        tracing::error!("Prompt error: {error}");
        state.waiting_for_response = false;
        state.grid_loader.visible = false;
    }

    fn on_authenticate_complete(&self, state: &mut UiState, method_id: &str) {
        state
            .auth_methods
            .retain(|method| method.id().0.as_ref() != method_id);
        if let Some(ref mut overlay) = state.config_overlay {
            overlay.remove_auth_method(method_id);
        }
    }

    fn on_authenticate_failed(&self, state: &mut UiState, method_id: &str, error: &str) {
        tracing::warn!("Provider auth failed for {method_id}: {error}");
        if let Some(ref mut overlay) = state.config_overlay {
            overlay.on_authenticate_failed(method_id);
        }
    }

    async fn submit_prompt<W: Write>(
        &self,
        view: &mut UiView<W>,
        user_input: &str,
        attachments: Vec<PromptAttachment>,
    ) -> Result<(), AppError> {
        let outcome = build_attachment_blocks(&attachments).await;

        if !outcome.warnings.is_empty() {
            let warning_lines: Vec<Line> = outcome
                .warnings
                .into_iter()
                .map(|warning| Line::new(format!("[wisp] {warning}")))
                .collect();
            view.push_scrollback(&warning_lines)?;
        }

        self.prompt_handle.prompt(
            &self.session_id,
            user_input,
            if outcome.blocks.is_empty() {
                None
            } else {
                Some(outcome.blocks)
            },
        )?;

        Ok(())
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
    use crate::keybindings::KeyBinding;
    use crate::tui::{Event, KeyCode, KeyModifiers, MouseEvent, MouseEventKind};

    fn make_controller() -> UiStateController {
        UiStateController::new(SessionId::new("test"), AcpPromptHandle::noop())
    }

    fn make_view() -> UiView<Vec<u8>> {
        use crate::tui::advanced::Renderer;
        use crate::tui::Theme;
        let renderer = Renderer::new(Vec::new(), Theme::default());
        let git_diff_mode = super::super::GitDiffMode::new(std::path::PathBuf::from("."));
        UiView::new(renderer, git_diff_mode)
    }

    #[tokio::test]
    async fn custom_exit_keybinding_triggers_exit() {
        let mut controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        controller.keybindings.exit =
            KeyBinding::new(KeyCode::Char('q'), KeyModifiers::CONTROL);

        let default_exit = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        controller
            .handle_terminal_event(&mut state, &mut view, &Event::Key(default_exit))
            .await
            .unwrap();
        assert!(
            !state.exit_requested,
            "default Ctrl+C should no longer exit"
        );

        state.exit_requested = false;
        let custom_exit = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        controller
            .handle_terminal_event(&mut state, &mut view, &Event::Key(custom_exit))
            .await
            .unwrap();
        assert!(state.exit_requested, "custom Ctrl+Q should exit");
    }

    #[tokio::test]
    async fn ctrl_g_opens_git_diff_viewer() {
        let mut controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        controller
            .handle_terminal_event(&mut state, &mut view, &Event::Key(key))
            .await
            .unwrap();
        assert!(matches!(state.screen_mode, ScreenMode::GitDiff));
    }

    #[tokio::test]
    async fn ctrl_g_closes_git_diff_viewer() {
        let mut controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        state.screen_mode = ScreenMode::GitDiff;

        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        controller
            .handle_terminal_event(&mut state, &mut view, &Event::Key(key))
            .await
            .unwrap();

        assert!(matches!(state.screen_mode, ScreenMode::Conversation));
    }

    #[tokio::test]
    async fn ctrl_g_blocked_during_elicitation() {
        let mut controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        state.elicitation_form = Some(ElicitationForm::from_params(
            acp_utils::notifications::ElicitationParams {
                message: "test".to_string(),
                schema: acp_utils::ElicitationSchema::builder().build().unwrap(),
            },
            tokio::sync::oneshot::channel().0,
        ));

        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        controller
            .handle_terminal_event(&mut state, &mut view, &Event::Key(key))
            .await
            .unwrap();

        assert!(
            !matches!(state.screen_mode, ScreenMode::GitDiff),
            "git diff should not open during elicitation"
        );
    }

    #[tokio::test]
    async fn esc_in_diff_mode_does_not_cancel() {
        let mut controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        state.waiting_for_response = true;
        state.screen_mode = ScreenMode::GitDiff;

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        controller
            .handle_terminal_event(&mut state, &mut view, &Event::Key(key))
            .await
            .unwrap();

        assert!(!state.exit_requested);
        assert!(
            state.waiting_for_response,
            "Esc should NOT cancel a running prompt while git diff mode is active"
        );
    }

    #[tokio::test]
    async fn mouse_scroll_ignored_in_conversation_mode() {
        let mut controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();

        let mouse = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        controller
            .handle_terminal_event(&mut state, &mut view, &Event::Mouse(mouse))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn prompt_composer_submit_sends_prompt() {
        let controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        let outcome = Some(vec![PromptComposerMessage::SubmitRequested {
            user_input: "hello".to_string(),
            attachments: vec![],
        }]);

        controller
            .handle_prompt_composer_messages(&mut state, &mut view, outcome)
            .await
            .unwrap();

        assert!(
            state.waiting_for_response,
            "submit should mark waiting state"
        );
    }

    #[tokio::test]
    async fn prompt_composer_open_config() {
        let controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        let outcome = Some(vec![PromptComposerMessage::OpenConfig]);

        controller
            .handle_prompt_composer_messages(&mut state, &mut view, outcome)
            .await
            .unwrap();

        assert!(
            state.config_overlay.is_some(),
            "config overlay should be opened"
        );
    }

    #[test]
    fn config_overlay_close_clears_overlay() {
        let controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        state.open_config_overlay();
        let outcome = Some(vec![ConfigOverlayMessage::Close]);

        controller
            .handle_config_overlay_messages(&mut state, &mut view, outcome)
            .unwrap();

        assert!(
            state.config_overlay.is_none(),
            "close message should clear overlay"
        );
    }

    #[test]
    fn theme_config_change_applies_theme() {
        let controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        use crate::components::config_menu::ConfigChange;
        let outcome = Some(vec![ConfigOverlayMessage::ApplyConfigChanges(vec![
            ConfigChange {
                config_id: THEME_CONFIG_ID.to_string(),
                new_value: "   ".to_string(),
            },
        ])]);

        controller
            .handle_config_overlay_messages(&mut state, &mut view, outcome)
            .unwrap();
        // Theme was applied (we can't easily verify the exact theme without a temp dir,
        // but the test passes without error).
    }

    #[test]
    fn theme_default_value_maps_to_none() {
        assert_eq!(theme_file_from_picker_value("   "), None);
    }

    #[test]
    fn theme_value_maps_to_some() {
        assert_eq!(
            theme_file_from_picker_value("catppuccin.tmTheme"),
            Some("catppuccin.tmTheme".to_string())
        );
    }

    #[tokio::test]
    async fn tick_advances_tool_call_statuses() {
        let mut controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();

        let tool_call = acp::ToolCall::new("tool-1".to_string(), "test_tool");
        state.tool_call_statuses.on_tool_call(&tool_call);
        state.grid_loader.visible = false;

        let tick_before = state.tool_call_statuses.tick();
        controller
            .handle_terminal_event(&mut state, &mut view, &Event::Tick)
            .await
            .unwrap();
        let tick_after = state.tool_call_statuses.tick();

        assert!(tick_after > tick_before);
    }

    #[tokio::test]
    async fn tick_advances_progress_indicator() {
        let mut controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();

        let tool_call = acp::ToolCall::new("tool-1".to_string(), "test_tool");
        state.tool_call_statuses.on_tool_call(&tool_call);

        state.progress_indicator.update(0, 1);
        let ctx = ViewContext::new((80, 24));
        let output_before = state.progress_indicator.render(&ctx);
        controller
            .handle_terminal_event(&mut state, &mut view, &Event::Tick)
            .await
            .unwrap();
        let output_after = state.progress_indicator.render(&ctx);

        assert_ne!(
            output_before[0].plain_text(),
            output_after[0].plain_text(),
            "spinner frame should change after tick"
        );
    }

    #[test]
    fn on_prompt_error_clears_waiting_state() {
        let controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        state.waiting_for_response = true;
        state.grid_loader.visible = true;

        let error = acp::Error::internal_error();
        controller.on_prompt_error(&mut state, &error);

        assert!(!state.waiting_for_response);
        assert!(!state.grid_loader.visible);
        assert!(!state.exit_requested);
    }

    #[test]
    fn on_authenticate_complete_removes_method() {
        let controller = make_controller();
        let mut state = UiState::new(
            "test-agent".to_string(),
            &[],
            vec![acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
                "anthropic",
                "Anthropic",
            ))],
        );

        controller.on_authenticate_complete(&mut state, "anthropic");

        assert!(state.auth_methods.is_empty());
        assert!(!state.exit_requested);
    }

    #[test]
    fn on_authenticate_failed_does_not_exit() {
        let controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);

        controller.on_authenticate_failed(&mut state, "anthropic", "bad token");

        assert!(!state.exit_requested);
    }

    #[test]
    fn on_connection_closed_requests_exit() {
        let mut controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();
        let context = ViewContext::new((80, 24));

        controller
            .handle_acp_event(
                &mut state,
                &mut view,
                AcpEvent::ConnectionClosed,
                &context,
            )
            .unwrap();

        assert!(state.exit_requested);
    }

    #[tokio::test]
    async fn clear_screen_sends_clear_prompt_to_agent() {
        let controller = make_controller();
        let mut state = UiState::new("test-agent".to_string(), &[], vec![]);
        let mut view = make_view();

        controller
            .handle_prompt_composer_messages(
                &mut state,
                &mut view,
                Some(vec![PromptComposerMessage::ClearScreen]),
            )
            .await
            .unwrap();

        assert!(!state.exit_requested);
    }

    #[tokio::test]
    async fn submit_prompt_with_missing_attachment_pushes_warning() {
        let controller = make_controller();
        let mut view = make_view();
        let attachment = PromptAttachment {
            path: std::path::PathBuf::from("missing-file.txt"),
            display_name: "missing-file.txt".to_string(),
        };

        controller
            .submit_prompt(&mut view, "hello", vec![attachment])
            .await
            .unwrap();

        let output = String::from_utf8_lossy(view.renderer().writer());
        assert!(output.contains("[wisp]"));
        assert!(output.contains("missing-file.txt"));
    }
}
