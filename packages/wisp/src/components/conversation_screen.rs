use crate::components::app::PromptAttachment;
use crate::components::command_picker::CommandEntry;
use crate::components::conversation_window::{ConversationBuffer, ConversationWindow};
use crate::components::elicitation_form::{ElicitationForm, ElicitationMessage};
use crate::components::plan_tracker::PlanTracker;
use crate::components::plan_view::PlanView;
use crate::components::progress_indicator::ProgressIndicator;
use crate::components::prompt_composer::{PromptComposer, PromptComposerMessage};
use crate::components::session_picker::{SessionEntry, SessionPicker, SessionPickerMessage};
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::keybindings::Keybindings;
use acp_utils::notifications::ElicitationResponse;
use agent_client_protocol::{self as acp, SessionId};
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::oneshot;
use tui::{Component, Event, Frame, Layout, ViewContext};

pub enum ConversationScreenMessage {
    SendPrompt {
        user_input: String,
        attachments: Vec<PromptAttachment>,
    },
    ClearScreen,
    NewSession,
    OpenSettings,
    OpenSessionPicker,
    LoadSession {
        session_id: SessionId,
        cwd: PathBuf,
    },
}

pub(crate) enum Modal {
    Elicitation(ElicitationForm),
    SessionPicker(SessionPicker),
}

pub struct ConversationScreen {
    pub(crate) conversation: ConversationBuffer,
    pub tool_call_statuses: ToolCallStatuses,
    pub(crate) prompt_composer: PromptComposer,
    pub(crate) plan_tracker: PlanTracker,
    pub(crate) progress_indicator: ProgressIndicator,
    pub(crate) waiting_for_response: bool,
    pub(crate) active_modal: Option<Modal>,
}

impl ConversationScreen {
    pub fn new(keybindings: Keybindings) -> Self {
        Self {
            conversation: ConversationBuffer::new(),
            tool_call_statuses: ToolCallStatuses::new(),
            prompt_composer: PromptComposer::new(keybindings),
            plan_tracker: PlanTracker::default(),
            progress_indicator: ProgressIndicator::default(),
            waiting_for_response: false,
            active_modal: None,
        }
    }

    pub fn has_modal(&self) -> bool {
        self.active_modal.is_some()
    }

    pub fn is_waiting(&self) -> bool {
        self.waiting_for_response
    }

    pub fn wants_tick(&self) -> bool {
        self.waiting_for_response
            || self.tool_call_statuses.progress().running_any
            || self.plan_tracker_has_tick_driven_visibility()
    }

    pub fn on_tick(&mut self, now: Instant) {
        self.tool_call_statuses.on_tick(now);
        self.plan_tracker.on_tick(now);
        self.progress_indicator.on_tick();
    }

    pub fn refresh_caches(&mut self, _context: &ViewContext) {
        let progress = self.tool_call_statuses.progress();
        self.progress_indicator.update(
            progress.completed_top_level,
            progress.total_top_level,
            self.waiting_for_response,
        );
        self.plan_tracker.cached_visible_entries();
    }

    pub fn reset_after_context_cleared(&mut self) {
        self.conversation.clear();
        self.tool_call_statuses.clear();
        self.waiting_for_response = false;
        self.plan_tracker.clear();
        self.progress_indicator = ProgressIndicator::default();
    }

    pub fn open_session_picker(&mut self, sessions: Vec<acp::SessionInfo>) {
        let entries = sessions.into_iter().map(SessionEntry).collect();
        self.active_modal = Some(Modal::SessionPicker(SessionPicker::new(entries)));
    }

    pub fn on_session_update(&mut self, update: &acp::SessionUpdate) {
        match update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                if let acp::ContentBlock::Text(text_content) = &chunk.content {
                    self.conversation.append_text_chunk(&text_content.text);
                }
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                if let acp::ContentBlock::Text(text_content) = &chunk.content {
                    self.conversation.append_thought_chunk(&text_content.text);
                }
            }
            acp::SessionUpdate::ToolCall(tool_call) => {
                self.conversation.close_thought_block();
                self.tool_call_statuses.on_tool_call(tool_call);
                self.conversation
                    .ensure_tool_segment(&tool_call.tool_call_id.0);
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                self.conversation.close_thought_block();
                self.tool_call_statuses.on_tool_call_update(update);
                if self.tool_call_statuses.has_tool(&update.tool_call_id.0) {
                    self.conversation
                        .ensure_tool_segment(&update.tool_call_id.0);
                }
            }
            acp::SessionUpdate::AvailableCommandsUpdate(update) => {
                let commands = update
                    .available_commands
                    .iter()
                    .map(|cmd| {
                        let hint = match cmd.input {
                            Some(acp::AvailableCommandInput::Unstructured(ref input)) => {
                                Some(input.hint.clone())
                            }
                            _ => None,
                        };
                        CommandEntry {
                            name: cmd.name.clone(),
                            description: cmd.description.clone(),
                            has_input: cmd.input.is_some(),
                            hint,
                            builtin: false,
                        }
                    })
                    .collect();
                self.prompt_composer.set_available_commands(commands);
            }
            acp::SessionUpdate::Plan(plan) => {
                self.plan_tracker
                    .replace(plan.entries.clone(), Instant::now());
            }
            _ => {
                self.conversation.close_thought_block();
            }
        }
    }

    pub fn on_prompt_done(&mut self, stop_reason: acp::StopReason) {
        self.waiting_for_response = false;
        self.tool_call_statuses
            .finalize_running(matches!(stop_reason, acp::StopReason::Cancelled));
        self.conversation.close_thought_block();
    }

    pub fn on_prompt_error(&mut self, error: &acp::Error) {
        tracing::error!("Prompt error: {error}");
        self.waiting_for_response = false;
    }

    pub fn on_elicitation_request(
        &mut self,
        params: acp_utils::notifications::ElicitationParams,
        response_tx: oneshot::Sender<ElicitationResponse>,
    ) {
        self.active_modal = Some(Modal::Elicitation(ElicitationForm::from_params(
            params,
            response_tx,
        )));
    }

    pub fn on_sub_agent_progress(
        &mut self,
        progress: &acp_utils::notifications::SubAgentProgressParams,
    ) {
        self.tool_call_statuses.on_sub_agent_progress(progress);
    }

    fn plan_tracker_has_tick_driven_visibility(&self) -> bool {
        self.plan_tracker.has_completed_in_grace_period()
    }

    async fn handle_modal_key(&mut self, event: &Event) -> Option<Vec<ConversationScreenMessage>> {
        let modal = self.active_modal.as_mut()?;
        match modal {
            Modal::Elicitation(form) => {
                let outcome = form.on_event(event).await;
                for msg in outcome.unwrap_or_default() {
                    match msg {
                        ElicitationMessage::Responded => {
                            self.active_modal = None;
                        }
                    }
                }
                Some(vec![])
            }
            Modal::SessionPicker(picker) => {
                let msgs = picker.on_event(event).await.unwrap_or_default();
                let mut out = Vec::new();
                for msg in msgs {
                    match msg {
                        SessionPickerMessage::Close => {
                            self.active_modal = None;
                        }
                        SessionPickerMessage::LoadSession { session_id, cwd } => {
                            self.active_modal = None;
                            self.reset_after_context_cleared();
                            out.push(ConversationScreenMessage::ClearScreen);
                            out.push(ConversationScreenMessage::LoadSession { session_id, cwd });
                        }
                    }
                }
                Some(out)
            }
        }
    }

    fn handle_prompt_composer_messages(
        &mut self,
        outcome: Option<Vec<PromptComposerMessage>>,
    ) -> Option<Vec<ConversationScreenMessage>> {
        let msgs = outcome?;
        let mut out = Vec::new();
        for msg in msgs {
            match msg {
                PromptComposerMessage::NewSession => {
                    self.reset_after_context_cleared();
                    out.push(ConversationScreenMessage::NewSession);
                }
                PromptComposerMessage::OpenSettings => {
                    out.push(ConversationScreenMessage::OpenSettings);
                }
                PromptComposerMessage::OpenSessionPicker => {
                    out.push(ConversationScreenMessage::OpenSessionPicker);
                }
                PromptComposerMessage::SubmitRequested {
                    user_input,
                    attachments,
                } => {
                    self.waiting_for_response = true;
                    out.push(ConversationScreenMessage::SendPrompt {
                        user_input,
                        attachments,
                    });
                }
            }
        }
        Some(out)
    }
}

impl Component for ConversationScreen {
    type Message = ConversationScreenMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<ConversationScreenMessage>> {
        if self.active_modal.is_some() {
            return self.handle_modal_key(event).await;
        }

        let composer_outcome = self.prompt_composer.on_event(event).await;
        if composer_outcome.is_some() {
            return self.handle_prompt_composer_messages(composer_outcome);
        }

        None
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let conversation_window = ConversationWindow {
            conversation: &self.conversation,
            tool_call_statuses: &self.tool_call_statuses,
        };
        let plan_view = PlanView {
            entries: self.plan_tracker.cached_entries(),
        };

        let mut layout = Layout::new();
        layout.section(conversation_window.render(ctx));
        layout.section(plan_view.render(ctx));
        layout.section(self.progress_indicator.render(ctx));
        let prompt_frame = self.prompt_composer.render(ctx);
        layout.section_with_cursor(prompt_frame.lines().to_vec(), prompt_frame.cursor());
        match &mut self.active_modal {
            Some(Modal::SessionPicker(picker)) => {
                let frame = picker.render(ctx);
                layout.section_with_cursor(frame.lines().to_vec(), frame.cursor());
            }
            Some(Modal::Elicitation(form)) => {
                layout.section(form.render(ctx).into_lines());
            }
            None => {}
        }
        layout.into_frame()
    }
}
