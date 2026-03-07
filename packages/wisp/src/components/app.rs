use crate::components::command_picker::{CommandEntry, CommandPicker, CommandPickerAction};
use crate::components::config_menu::ConfigMenu;
use crate::components::config_overlay::{ConfigOverlay, ConfigOverlayAction};
use crate::components::container::Container;
#[cfg(test)]
use crate::components::conversation_window::SegmentContent;
use crate::components::conversation_window::{ConversationBuffer, ConversationWindow};
use crate::components::elicitation_form::ElicitationForm;
use crate::components::file_picker::{FileMatch, FilePicker, FilePickerAction};
use crate::components::input_prompt::InputPrompt;
use crate::components::plan_view::PlanView;
use crate::components::progress_indicator::ProgressIndicator;
use crate::components::server_status::server_status_summary;
use crate::components::status_line::StatusLine;
use crate::components::text_input::{SelectedFileMention, TextInput, TextInputAction};
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::settings::{list_theme_files, load_or_create_settings};
use crate::tui::spinner::Spinner;
use crate::tui::{
    Cursor, CursorComponent, FormAction, HandlesInput, InputOutcome, Line, RenderContext,
    RenderOutput,
};
use acp_utils::config_option_id::{ConfigOptionId, THEME_CONFIG_ID};
use acp_utils::notifications::{
    CONTEXT_CLEARED_METHOD, CONTEXT_USAGE_METHOD, ContextUsageParams, ElicitationParams,
    ElicitationResponse, McpNotification, McpServerStatus, McpServerStatusEntry,
    SUB_AGENT_PROGRESS_METHOD, SubAgentProgressParams,
};
use agent_client_protocol::{
    self as acp, ExtNotification, SessionConfigKind, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigSelectOptions, SessionUpdate,
};
use crossterm::event::{self, KeyCode, KeyEvent};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tokio::sync::oneshot;
use url::Url;
use utils::ReasoningEffort;

const MAX_EMBED_TEXT_BYTES: usize = 1024 * 1024;

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

#[derive(Debug, Default)]
pub struct AttachmentBuildOutcome {
    pub blocks: Vec<acp::ContentBlock>,
    pub warnings: Vec<String>,
}

pub struct App {
    tool_call_statuses: ToolCallStatuses,
    grid_loader: Spinner,
    conversation: ConversationBuffer,
    pub(crate) text_input: TextInput,
    agent_name: String,
    mode_display: Option<String>,
    model_display: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    config_options: Vec<SessionConfigOption>,
    waiting_for_response: bool,
    animation_tick: u16,
    available_commands: Vec<CommandEntry>,
    context_usage_pct: Option<u8>,
    file_picker: Option<FilePicker>,
    command_picker: Option<CommandPicker>,
    config_overlay: Option<ConfigOverlay>,
    elicitation_form: Option<ElicitationForm>,
    server_statuses: Vec<McpServerStatusEntry>,
    auth_methods: Vec<acp::AuthMethod>,
    plan_entries: Vec<acp::PlanEntry>,
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
            text_input: TextInput::new(),
            agent_name,
            mode_display: extract_mode_display(config_options),
            model_display: extract_model_display(config_options),
            reasoning_effort: extract_reasoning_effort(config_options),
            config_options: config_options.to_vec(),
            waiting_for_response: false,
            animation_tick: 0,
            available_commands: Vec::new(),
            context_usage_pct: None,
            file_picker: None,
            command_picker: None,
            config_overlay: None,
            elicitation_form: None,
            server_statuses: Vec::new(),
            auth_methods,
            plan_entries: Vec::new(),
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

        if let Some(effects) = self.handle_picker_key(key_event) {
            return effects;
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

        // Swallow cursor keys when file picker overlay is open.
        if self.file_picker.is_some()
            && matches!(
                key_event.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
            )
        {
            return vec![];
        }

        let outcome = self.text_input.handle_key(key_event);
        self.handle_text_input_outcome(&outcome)
    }

    pub fn on_session_update(&mut self, update: SessionUpdate) -> Vec<AppEvent> {
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
                self.available_commands = update
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
            }
            SessionUpdate::ConfigOptionUpdate(update) => {
                self.conversation.close_thought_block();
                self.mode_display = extract_mode_display(&update.config_options);
                self.model_display = extract_model_display(&update.config_options);
                self.reasoning_effort = extract_reasoning_effort(&update.config_options);
                self.config_options.clone_from(&update.config_options);
                if let Some(ref mut overlay) = self.config_overlay {
                    overlay.update_config_options(&update.config_options);
                }
                should_render = true;
            }
            SessionUpdate::Plan(plan) => {
                self.plan_entries = plan.entries;
                should_render = true;
            }
            _ => {
                self.conversation.close_thought_block();
            }
        }

        if should_render {
            vec![AppEvent::Render]
        } else {
            vec![]
        }
    }

    pub fn on_prompt_done(&mut self, context: &RenderContext) -> Vec<AppEvent> {
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
            effects.push(AppEvent::PushScrollback(scrollback_lines));
        }
        effects.push(AppEvent::Render);
        effects
    }

    pub fn on_tick(&mut self) -> Vec<AppEvent> {
        let has_in_progress_plan = self
            .plan_entries
            .iter()
            .any(|e| e.status == acp::PlanEntryStatus::InProgress);
        if self.waiting_for_response
            || self.tool_call_statuses.progress().running_any
            || has_in_progress_plan
        {
            self.animation_tick = self.animation_tick.wrapping_add(1);
            self.grid_loader.tick = self.animation_tick;
            self.tool_call_statuses.set_tick(self.animation_tick);
            vec![AppEvent::Render]
        } else {
            vec![]
        }
    }

    pub fn on_elicitation_request(
        &mut self,
        params: ElicitationParams,
        response_tx: oneshot::Sender<ElicitationResponse>,
    ) -> Vec<AppEvent> {
        self.config_overlay = None;
        self.elicitation_form = Some(ElicitationForm::from_params(params, response_tx));
        vec![AppEvent::Render]
    }

    pub fn on_ext_notification(&mut self, notification: &ExtNotification) -> Vec<AppEvent> {
        match notification.method.as_ref() {
            CONTEXT_CLEARED_METHOD => {
                self.reset_after_context_cleared();
                return vec![AppEvent::Render];
            }
            CONTEXT_USAGE_METHOD => {
                if let Ok(params) =
                    serde_json::from_str::<ContextUsageParams>(notification.params.get())
                {
                    self.context_usage_pct = params.usage_ratio.map(|usage_ratio| {
                        // Safety: clamp guarantees value is in [0.0, 100.0], round() keeps it integral
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let pct_left =
                            ((1.0 - usage_ratio) * 100.0).clamp(0.0, 100.0).round() as u8;
                        pct_left
                    });
                    return vec![AppEvent::Render];
                }
            }
            SUB_AGENT_PROGRESS_METHOD => {
                if let Ok(progress) =
                    serde_json::from_str::<SubAgentProgressParams>(notification.params.get())
                {
                    self.tool_call_statuses.on_sub_agent_progress(&progress);
                    return vec![AppEvent::Render];
                }
            }
            _ => {
                if let Ok(McpNotification::ServerStatus { servers }) =
                    McpNotification::try_from(notification)
                {
                    self.server_statuses.clone_from(&servers);
                    if let Some(ref mut overlay) = self.config_overlay {
                        overlay.update_server_statuses(servers);
                    }
                    return vec![AppEvent::Render];
                }
            }
        }
        vec![]
    }

    fn reset_after_context_cleared(&mut self) {
        self.conversation.clear();
        self.tool_call_statuses.clear();
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        self.animation_tick = 0;
        self.context_usage_pct = None;
        self.plan_entries.clear();
    }

    pub fn on_prompt_error(&mut self) -> Vec<AppEvent> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        vec![AppEvent::Render]
    }

    pub fn on_paste(&mut self, text: &str) -> Vec<AppEvent> {
        self.close_all_pickers();
        self.text_input.insert_paste(text);
        vec![AppEvent::Render]
    }

    pub fn on_resize(_cols: u16, _rows: u16) -> Vec<AppEvent> {
        vec![AppEvent::Render]
    }

    #[allow(dead_code)]
    pub fn has_file_picker(&self) -> bool {
        self.file_picker.is_some()
    }

    #[allow(dead_code)]
    pub fn has_command_picker(&self) -> bool {
        self.command_picker.is_some()
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
            .and_then(|o| o.picker_config_id())
    }

    #[allow(dead_code)]
    pub fn file_picker_selected_display_name(&self) -> Option<String> {
        self.file_picker
            .as_ref()
            .and_then(|p| p.selected().map(|f| f.display_name.clone()))
    }

    #[allow(dead_code)]
    pub fn command_picker_match_names(&self) -> Vec<&str> {
        self.command_picker
            .as_ref()
            .map(|p| p.matches().iter().map(|m| m.name.as_str()).collect())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn open_file_picker_with_matches(&mut self, matches: Vec<FileMatch>) {
        self.file_picker = Some(FilePicker::from_matches(matches));
    }

    #[allow(dead_code)]
    pub fn available_commands(&self) -> &[CommandEntry] {
        &self.available_commands
    }

    pub fn on_authenticate_started(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.on_authenticate_started(method_id);
        }
    }

    pub fn on_authenticate_complete(&mut self, method_id: &str) {
        self.auth_methods.retain(|m| m.id.0.as_ref() != method_id);
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.remove_auth_method(method_id);
        }
    }

    pub fn on_authenticate_failed(&mut self, method_id: &str) {
        if let Some(ref mut overlay) = self.config_overlay {
            overlay.on_authenticate_failed(method_id);
        }
    }

    fn handle_elicitation_key(&mut self, key_event: KeyEvent) -> Option<Vec<AppEvent>> {
        let ef = self.elicitation_form.as_mut()?;
        let outcome = ef.form.handle_key(key_event);

        match outcome.action {
            Some(FormAction::Close) => {
                if let Some(ef) = self.elicitation_form.take() {
                    let _ = ef.response_tx.send(ElicitationForm::decline());
                }
            }
            Some(FormAction::Submit) => {
                if let Some(ef) = self.elicitation_form.take() {
                    let response = ef.confirm();
                    let _ = ef.response_tx.send(response);
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

    fn handle_picker_key(&mut self, key_event: KeyEvent) -> Option<Vec<AppEvent>> {
        if let Some(ref mut picker) = self.file_picker {
            let outcome = picker.handle_key(key_event);
            if outcome.consumed {
                return Some(self.handle_file_picker_outcome(&outcome));
            }
        }

        if let Some(ref mut picker) = self.command_picker {
            let outcome = picker.handle_key(key_event);
            return Some(self.handle_command_picker_outcome(&outcome));
        }

        if let Some(ref mut overlay) = self.config_overlay {
            let outcome = overlay.handle_key(key_event);
            return Some(self.handle_config_overlay_outcome(outcome));
        }

        None
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
            .any(|o| o.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str())
            .then(|| {
                let next = ReasoningEffort::cycle_next(self.reasoning_effort);
                AppEvent::SetConfigOption {
                    config_id: ConfigOptionId::ReasoningEffort.as_str().to_string(),
                    new_value: ReasoningEffort::config_str(next).to_string(),
                }
            })
    }

    fn handle_file_picker_outcome(
        &mut self,
        outcome: &InputOutcome<FilePickerAction>,
    ) -> Vec<AppEvent> {
        match outcome.action {
            Some(FilePickerAction::Close) => {
                self.file_picker = None;
            }
            Some(FilePickerAction::CloseAndPopChar) => {
                self.text_input.delete_char_before_cursor();
                self.file_picker = None;
            }
            Some(FilePickerAction::CloseWithChar(c)) => {
                self.text_input.insert_char_at_cursor(c);
                self.file_picker = None;
            }
            Some(FilePickerAction::ConfirmSelection) => {
                let selected = self.file_picker.take().and_then(|p| p.selected().cloned());
                if let Some(selected) = selected {
                    self.text_input
                        .apply_file_selection(selected.path, selected.display_name);
                }
            }
            Some(FilePickerAction::CharTyped(c)) => {
                self.text_input.insert_char_at_cursor(c);
            }
            Some(FilePickerAction::PopChar) => {
                self.text_input.delete_char_before_cursor();
            }
            None => {}
        }

        if outcome.needs_render {
            vec![AppEvent::Render]
        } else {
            vec![]
        }
    }

    fn handle_command_picker_outcome(
        &mut self,
        outcome: &InputOutcome<CommandPickerAction>,
    ) -> Vec<AppEvent> {
        match outcome.action {
            Some(CommandPickerAction::Close) => {
                self.command_picker = None;
            }
            Some(CommandPickerAction::CloseAndPopChar) => {
                self.text_input.delete_char_before_cursor();
                self.command_picker = None;
            }
            Some(CommandPickerAction::CloseWithChar(c)) => {
                self.text_input.insert_char_at_cursor(c);
                self.command_picker = None;
            }
            Some(CommandPickerAction::CommandChosen(ref cmd)) => {
                self.command_picker = None;
                return self.apply_command(cmd);
            }
            Some(CommandPickerAction::CharTyped(c)) => {
                self.text_input.insert_char_at_cursor(c);
            }
            Some(CommandPickerAction::PopChar) => {
                self.text_input.delete_char_before_cursor();
            }
            None => {}
        }

        if outcome.needs_render {
            vec![AppEvent::Render]
        } else {
            vec![]
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
                for c in changes {
                    if c.config_id == THEME_CONFIG_ID {
                        events.push(AppEvent::SetTheme {
                            file: theme_file_from_picker_value(&c.new_value),
                        });
                    } else {
                        events.push(AppEvent::SetConfigOption {
                            config_id: c.config_id,
                            new_value: c.new_value,
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
            None => {
                if outcome.needs_render {
                    vec![AppEvent::Render]
                } else {
                    vec![]
                }
            }
        }
    }

    fn apply_command(&mut self, cmd: &CommandEntry) -> Vec<AppEvent> {
        if cmd.builtin && cmd.name == "config" {
            self.text_input.clear();
            self.close_all_pickers();
            self.open_config_overlay();
            vec![AppEvent::Render]
        } else if cmd.has_input {
            self.text_input.set_input(format!("/{} ", cmd.name));
            vec![AppEvent::Render]
        } else {
            self.text_input.set_input(format!("/{}", cmd.name));
            self.execute_input()
        }
    }

    fn execute_input(&mut self) -> Vec<AppEvent> {
        if self.text_input.buffer().trim().is_empty() {
            return vec![AppEvent::Render];
        }

        let user_input = self.text_input.buffer().trim().to_string();
        let attachments = collect_submit_attachments(&user_input, self.text_input.take_mentions());
        self.text_input.clear();
        self.close_input_pickers();

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

    fn handle_text_input_outcome(
        &mut self,
        outcome: &InputOutcome<TextInputAction>,
    ) -> Vec<AppEvent> {
        match outcome.action {
            Some(TextInputAction::Submit) => self.execute_input(),
            Some(TextInputAction::OpenCommandPicker) => {
                let mut commands = builtin_commands();
                commands.extend(self.available_commands.clone());
                self.open_command_picker(commands);
                vec![AppEvent::Render]
            }
            Some(TextInputAction::OpenFilePicker) => {
                self.open_file_picker();
                vec![AppEvent::Render]
            }
            None if outcome.needs_render => vec![AppEvent::Render],
            _ => vec![],
        }
    }

    fn open_file_picker(&mut self) {
        self.file_picker = Some(FilePicker::new());
    }

    fn open_command_picker(&mut self, commands: Vec<CommandEntry>) {
        self.command_picker = Some(CommandPicker::new(commands));
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

    fn close_all_pickers(&mut self) {
        self.file_picker = None;
        self.command_picker = None;
        self.config_overlay = None;
    }

    fn close_input_pickers(&mut self) {
        self.file_picker = None;
        self.command_picker = None;
    }
}

fn collect_submit_attachments(
    user_input: &str,
    selected_mentions: Vec<SelectedFileMention>,
) -> Vec<PromptAttachment> {
    let mentions: HashSet<&str> = user_input.split_whitespace().collect();
    selected_mentions
        .into_iter()
        .filter(|mention| mentions.contains(mention.mention.as_str()))
        .map(|mention| PromptAttachment {
            path: mention.path,
            display_name: mention.display_name,
        })
        .collect()
}

pub async fn build_attachment_blocks(attachments: &[PromptAttachment]) -> AttachmentBuildOutcome {
    let mut outcome = AttachmentBuildOutcome::default();

    for attachment in attachments {
        match try_build_attachment_block(&attachment.path, &attachment.display_name).await {
            Ok((block, maybe_warning)) => {
                outcome.blocks.push(block);
                if let Some(warning) = maybe_warning {
                    outcome.warnings.push(warning);
                }
            }
            Err(warning) => outcome.warnings.push(warning),
        }
    }

    outcome
}

async fn try_build_attachment_block(
    path: &Path,
    display_name: &str,
) -> Result<(acp::ContentBlock, Option<String>), String> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("Failed to read {display_name}: {e}"))?;

    let mut bytes = Vec::new();
    file.take((MAX_EMBED_TEXT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| format!("Failed to read {display_name}: {e}"))?;

    let truncated = bytes.len() > MAX_EMBED_TEXT_BYTES;
    if truncated {
        bytes.truncate(MAX_EMBED_TEXT_BYTES);
    }
    let text_bytes = bytes.as_slice();

    let text = match std::str::from_utf8(text_bytes) {
        Ok(text) => text.to_string(),
        Err(error) if truncated && error.valid_up_to() > 0 => {
            let valid_bytes = &text_bytes[..error.valid_up_to()];
            std::str::from_utf8(valid_bytes)
                .expect("valid_up_to must point at a utf8 boundary")
                .to_string()
        }
        Err(_) => return Err(format!("Skipped binary or non-UTF8 file: {display_name}")),
    };

    let file_uri = build_attachment_file_uri(path, display_name).await?;

    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    let warning =
        truncated.then(|| format!("Truncated {display_name} to {MAX_EMBED_TEXT_BYTES} bytes"));

    let block = acp::ContentBlock::Resource(acp::EmbeddedResource::new(
        acp::EmbeddedResourceResource::TextResourceContents(
            acp::TextResourceContents::new(text, file_uri).mime_type(mime_type),
        ),
    ));

    Ok((block, warning))
}

async fn build_attachment_file_uri(path: &Path, display_name: &str) -> Result<String, String> {
    let canonical_path = tokio::fs::canonicalize(path).await.ok();
    let uri_path = canonical_path.as_deref().unwrap_or(path);
    let file_uri = Url::from_file_path(uri_path)
        .map_err(|()| format!("Failed to build file URI for {display_name}"))?
        .to_string();
    Ok(file_uri)
}

impl CursorComponent for App {
    fn render_with_cursor(&mut self, context: &RenderContext) -> RenderOutput {
        let unhealthy_count = self
            .server_statuses
            .iter()
            .filter(|s| !matches!(s.status, McpServerStatus::Connected { .. }))
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

        // Full-screen config overlay path
        if let Some(ref mut overlay) = self.config_overlay {
            let cursor_col = overlay.cursor_col();
            let cursor_row = overlay.cursor_row_offset();

            let mut container = Container::new(vec![overlay, &mut status_line]);
            let (lines, _offsets) = container.render_with_offsets(context);

            let cursor = Cursor {
                logical_row: cursor_row,
                col: cursor_col,
            };

            return RenderOutput {
                lines,
                cursor,
                cursor_visible: overlay.has_picker(),
            };
        }

        // Normal rendering path
        let picker_query_len = self.file_picker.as_ref().map(|p| p.query().len());
        let cursor_index = self.text_input.cursor_index(picker_query_len);

        let mut conversation_window = ConversationWindow {
            loader: &mut self.grid_loader,
            conversation: &mut self.conversation,
            tool_call_statuses: &self.tool_call_statuses,
        };
        let mut input_prompt = InputPrompt {
            input: self.text_input.buffer(),
            cursor_index,
        };
        let input_layout = input_prompt.layout(context);

        let mut plan_view = PlanView {
            entries: &self.plan_entries,
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
            &mut input_prompt,
        ]);
        let input_component_index = container.len() - 1;

        if let Some(ref mut picker) = self.file_picker {
            container.push(picker);
        }

        if let Some(ref mut picker) = self.command_picker {
            container.push(picker);
        }

        if let Some(ref mut ef) = self.elicitation_form {
            container.push(&mut ef.form);
        }

        container.push(&mut status_line);
        let (lines, offsets) = container.render_with_offsets(context);

        let cursor = Cursor {
            logical_row: offsets[input_component_index] + input_layout.cursor_row,
            col: input_layout.cursor_col as usize,
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

fn builtin_commands() -> Vec<CommandEntry> {
    vec![CommandEntry {
        name: "config".into(),
        description: "Open configuration settings".into(),
        has_input: false,
        hint: None,
        builtin: true,
    }]
}

fn option_display_name(
    options: &SessionConfigSelectOptions,
    current_value: &acp::SessionConfigValueId,
) -> Option<String> {
    match options {
        SessionConfigSelectOptions::Ungrouped(options) => options
            .iter()
            .find(|o| &o.value == current_value)
            .map(|o| o.name.clone()),
        SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| group.options.iter())
            .find(|o| &o.value == current_value)
            .map(|o| o.name.clone()),
        _ => None,
    }
}

fn extract_select_display(
    config_options: &[SessionConfigOption],
    id: ConfigOptionId,
) -> Option<String> {
    let option = config_options
        .iter()
        .find(|o| o.id.0.as_ref() == id.as_str())?;

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
        .find(|o| o.id.0.as_ref() == ConfigOptionId::Model.as_str())?;

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
        // Multi-select model — look up each component's display name
        let names: Vec<&str> = current
            .split(',')
            .filter_map(|part| {
                let trimmed = part.trim();
                options
                    .iter()
                    .find(|o| o.value.0.as_ref() == trimmed)
                    .map(|o| o.name.as_str())
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
        .find(|o| o.id.0.as_ref() == ConfigOptionId::ReasoningEffort.as_str())?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    ReasoningEffort::parse(&select.current_value.0).unwrap_or(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::config_menu::ConfigChange;
    use crate::settings::{ThemeSettings as WispThemeSettings, WispSettings, save_settings};
    use crate::test_helpers::{CUSTOM_TMTHEME, with_wisp_home};
    use crate::tui::Component;
    use crossterm::event::KeyModifiers;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

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
                    .any(|v| v.value == "catppuccin.tmTheme")
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
                ConfigChange {
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
                ConfigChange {
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
                ConfigChange {
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
        screen.command_picker = Some(CommandPicker::new(vec![CommandEntry {
            name: "config".to_string(),
            description: "Open config".to_string(),
            has_input: false,
            hint: None,
            builtin: true,
        }]));

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
        let opts = vec![agent_client_protocol::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![agent_client_protocol::SessionConfigSelectOption::new(
                "m1", "M1",
            )],
        )];
        let mut screen = App::new("test-agent".to_string(), &opts, vec![]);
        screen.open_config_overlay();

        let context = RenderContext::new((120, 40));
        let output = screen.render_with_cursor(&context);

        // The overlay should contain the bordered Configuration title
        let has_config_title = output
            .lines
            .iter()
            .any(|line| line.plain_text().contains("Configuration"));
        assert!(has_config_title, "overlay should show Configuration title");

        // Closing the overlay should restore normal layout
        screen.config_overlay = None;
        let output2 = screen.render_with_cursor(&context);
        let has_config_title2 = output2
            .lines
            .iter()
            .any(|line| line.plain_text().contains("Configuration"));
        assert!(
            !has_config_title2,
            "normal layout should not show Configuration title"
        );
    }

    #[test]
    fn builtin_config_command_opens_config_menu() {
        let options = vec![agent_client_protocol::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![agent_client_protocol::SessionConfigSelectOption::new(
                "m1", "M1",
            )],
        )];
        let mut screen = App::new("test-agent".to_string(), &options, vec![]);
        let effects = screen.apply_command(&CommandEntry {
            name: "config".to_string(),
            description: "Open configuration settings".to_string(),
            has_input: false,
            hint: None,
            builtin: true,
        });

        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
        assert!(screen.has_config_overlay());
        assert_eq!(screen.text_input.buffer(), "");
    }

    #[test]
    fn command_without_input_submits_prompt_immediately() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        let effects = screen.apply_command(&CommandEntry {
            name: "status".to_string(),
            description: "status".to_string(),
            has_input: false,
            hint: None,
            builtin: false,
        });

        assert!(effects
            .iter()
            .any(|effect| matches!(effect, AppEvent::PromptSubmit { user_input, .. } if user_input == "/status")));
        assert!(screen.waiting_for_response);
        assert!(screen.grid_loader.visible);
    }

    #[test]
    fn file_selection_updates_mentions_and_input() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen.text_input.set_input("@fo".to_string());

        screen
            .text_input
            .apply_file_selection(PathBuf::from("foo.rs"), "foo.rs".to_string());

        assert_eq!(screen.text_input.buffer(), "@foo.rs ");
    }

    #[test]
    fn config_with_single_option_shows_menu_not_picker() {
        let options = vec![agent_client_protocol::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![
                agent_client_protocol::SessionConfigSelectOption::new("m1", "M1"),
                agent_client_protocol::SessionConfigSelectOption::new("m2", "M2"),
            ],
        )];
        let mut screen = App::new("test-agent".to_string(), &options, vec![]);
        let effects = screen.apply_command(&CommandEntry {
            name: "config".to_string(),
            description: "Open configuration settings".to_string(),
            has_input: false,
            hint: None,
            builtin: true,
        });

        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
        assert!(screen.has_config_overlay(), "Config overlay should be open");
        assert!(
            !screen.has_config_picker(),
            "Picker should not auto-open; user navigates menu first"
        );
    }

    #[test]
    fn prompt_done_keeps_running_tool_segment() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);

        let tool_call = acp::ToolCall::new("tool-1", "Read file");
        screen.tool_call_statuses.on_tool_call(&tool_call);
        screen.conversation.ensure_tool_segment("tool-1");

        let effects = screen.on_prompt_done(&RenderContext::new((120, 40)));

        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
        let segments: Vec<_> = screen.conversation.segments().collect();
        assert!(matches!(segments[..], [SegmentContent::ToolCall(id)] if id == "tool-1"));
    }

    #[test]
    fn prompt_done_flush_respects_custom_theme() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen
            .conversation
            .append_thought_chunk("theme should be preserved");

        let custom_theme = custom_theme();
        let context = RenderContext::new((120, 40)).with_theme(custom_theme.clone());
        let effects = screen.on_prompt_done(&context);

        let AppEvent::PushScrollback(lines) = &effects[0] else {
            panic!("expected first effect to push scrollback");
        };
        let first_span = lines
            .first()
            .and_then(|line| line.spans().first())
            .expect("thought line should include spans");
        assert_eq!(
            first_span.style().fg,
            Some(custom_theme.muted()),
            "scrollback flush should render with active theme muted color"
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
                .any(|event| matches!(event, AppEvent::SetConfigOption { .. })),
            "overlay should consume shift+tab"
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
    fn streaming_chunks_keep_waiting_for_response() {
        let mut screen = App::new("test-agent".to_string(), &[], vec![]);
        screen.waiting_for_response = true;

        // Simulate a streaming text chunk arriving
        screen.on_session_update(SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
            acp::ContentBlock::Text(acp::TextContent::new("hello")),
        )));

        assert!(
            screen.waiting_for_response,
            "waiting_for_response should remain true while streaming"
        );

        // ESC should still emit Cancel
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
    fn sub_agent_progress_notification_triggers_render() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let json = r#"{"parent_tool_id":"p1","task_id":"t1","agent_name":"explorer","event":{"ToolCall":{"request":{"id":"c1","name":"grep","arguments":"{}"},"model_name":"m"}}}"#;
        let raw = serde_json::value::to_raw_value(
            &serde_json::from_str::<serde_json::Value>(json).unwrap(),
        )
        .unwrap();
        let notification =
            acp::ExtNotification::new("_aether/sub_agent_progress", std::sync::Arc::from(raw));

        let effects = app.on_ext_notification(&notification);
        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
    }

    #[test]
    fn invalid_sub_agent_progress_json_silently_ignored() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        let raw = serde_json::value::to_raw_value(&serde_json::json!({"bad": "data"})).unwrap();
        let notification =
            acp::ExtNotification::new("_aether/sub_agent_progress", std::sync::Arc::from(raw));

        let effects = app.on_ext_notification(&notification);
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
        let notification =
            acp::ExtNotification::new(CONTEXT_USAGE_METHOD, std::sync::Arc::from(raw));

        let effects = app.on_ext_notification(&notification);

        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
        assert_eq!(app.context_usage_pct, Some(25));
    }

    #[test]
    fn context_usage_notification_with_unknown_limit_clears_meter() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        app.context_usage_pct = Some(33);

        let raw = serde_json::value::to_raw_value(&serde_json::json!({
            "usage_ratio": null,
            "tokens_used": 0,
            "context_limit": null
        }))
        .unwrap();
        let notification =
            acp::ExtNotification::new(CONTEXT_USAGE_METHOD, std::sync::Arc::from(raw));

        let effects = app.on_ext_notification(&notification);

        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
        assert_eq!(app.context_usage_pct, None);
    }

    #[test]
    fn context_cleared_notification_resets_conversation_and_tool_state() {
        let mut app = App::new("test-agent".to_string(), &[], vec![]);
        app.waiting_for_response = true;
        app.grid_loader.visible = true;
        app.context_usage_pct = Some(25);
        app.conversation
            .set_segments(vec![SegmentContent::Text("hello".to_string())]);
        app.tool_call_statuses
            .on_tool_call(&acp::ToolCall::new("tool-1", "Read file"));

        let raw = serde_json::value::to_raw_value(&serde_json::json!({})).unwrap();
        let notification =
            acp::ExtNotification::new(CONTEXT_CLEARED_METHOD, std::sync::Arc::from(raw));

        let effects = app.on_ext_notification(&notification);

        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
        assert!(!app.waiting_for_response);
        assert!(!app.grid_loader.visible);
        assert_eq!(app.context_usage_pct, None);
        assert_eq!(app.conversation.segments().len(), 0);
        assert_eq!(app.tool_call_statuses.progress().total_top_level, 0);
    }

    #[test]
    fn paste_inserts_at_cursor_position() {
        let mut app = App::new("test".to_string(), &[], vec![]);
        app.text_input.set_input("hd".to_string());
        // Move cursor to position 1
        app.on_key_event(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        app.on_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        app.on_paste("ello worl");

        assert_eq!(app.text_input.buffer(), "hello world");
        assert_eq!(app.text_input.cursor_index(None), 10);
    }

    #[test]
    fn execute_resets_cursor_pos() {
        let mut app = App::new("test".to_string(), &[], vec![]);
        app.text_input.set_input("hello".to_string());

        app.execute_input();

        assert_eq!(app.text_input.cursor_index(None), 0);
        assert!(app.text_input.buffer().is_empty());
    }

    #[test]
    fn execute_input_emits_render_before_prompt_submit() {
        let mut app = App::new("test".to_string(), &[], vec![]);
        app.text_input.set_input("hello".to_string());

        let effects = app.execute_input();
        let render_pos = effects
            .iter()
            .position(|effect| matches!(effect, AppEvent::Render))
            .expect("render effect should be present");
        let submit_pos = effects
            .iter()
            .position(|effect| matches!(effect, AppEvent::PromptSubmit { .. }))
            .expect("prompt submit effect should be present");

        assert!(render_pos < submit_pos);
    }

    #[test]
    fn collect_submit_attachments_filters_unmentioned_files() {
        let selected = vec![
            SelectedFileMention {
                mention: "@keep.rs".to_string(),
                path: PathBuf::from("/tmp/keep.rs"),
                display_name: "keep.rs".to_string(),
            },
            SelectedFileMention {
                mention: "@skip.rs".to_string(),
                path: PathBuf::from("/tmp/skip.rs"),
                display_name: "skip.rs".to_string(),
            },
        ];

        let attachments = collect_submit_attachments("inspect @keep.rs now", selected);

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].display_name, "keep.rs");
        assert_eq!(attachments[0].path, PathBuf::from("/tmp/keep.rs"));
    }

    #[tokio::test]
    async fn build_attachment_blocks_truncates_large_file_with_warning() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("large.txt");
        let display_name = "large.txt".to_string();
        std::fs::write(&path, "x".repeat(MAX_EMBED_TEXT_BYTES + 64)).unwrap();

        let attachments = vec![PromptAttachment {
            path,
            display_name: display_name.clone(),
        }];
        let blocks = build_attachment_blocks(&attachments).await;

        assert_eq!(blocks.blocks.len(), 1);
        assert_eq!(blocks.warnings.len(), 1);
        assert!(blocks.warnings[0].contains(&format!(
            "Truncated {display_name} to {MAX_EMBED_TEXT_BYTES} bytes"
        )));
    }

    #[tokio::test]
    async fn build_attachment_blocks_skips_non_utf8_files() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("binary.bin");
        let display_name = "binary.bin".to_string();
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();

        let attachments = vec![PromptAttachment {
            path,
            display_name: display_name.clone(),
        }];
        let blocks = build_attachment_blocks(&attachments).await;

        assert!(blocks.blocks.is_empty());
        assert_eq!(blocks.warnings.len(), 1);
        assert!(
            blocks.warnings[0]
                .contains(&format!("Skipped binary or non-UTF8 file: {display_name}"))
        );
    }

    #[tokio::test]
    async fn build_attachment_file_uri_falls_back_when_canonicalize_fails() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("missing.txt");
        let expected = Url::from_file_path(&path).unwrap().to_string();

        let uri = build_attachment_file_uri(&path, "missing.txt")
            .await
            .expect("URI should be built from original absolute path");

        assert_eq!(uri, expected);
    }

    #[test]
    fn extract_model_display_handles_comma_separated_value() {
        let opts = vec![SessionConfigOption::select(
            "model",
            "Model",
            "a:x,b:y",
            vec![
                acp::SessionConfigSelectOption::new("a:x", "Alpha / X"),
                acp::SessionConfigSelectOption::new("b:y", "Beta / Y"),
                acp::SessionConfigSelectOption::new("c:z", "Gamma / Z"),
            ],
        )];
        let display = extract_model_display(&opts).expect("display");
        assert_eq!(display, "Alpha / X + Beta / Y");
    }

    #[test]
    fn extract_model_display_single_value() {
        let opts = vec![SessionConfigOption::select(
            "model",
            "Model",
            "a:x",
            vec![
                acp::SessionConfigSelectOption::new("a:x", "Alpha / X"),
                acp::SessionConfigSelectOption::new("b:y", "Beta / Y"),
            ],
        )];
        let display = extract_model_display(&opts).expect("display");
        assert_eq!(display, "Alpha / X");
    }

    #[test]
    fn multi_select_model_entry_routes_to_model_selector() {
        let mut meta = serde_json::Map::new();
        meta.insert("multi_select".to_string(), serde_json::Value::Bool(true));
        let opts = vec![
            SessionConfigOption::select(
                "model",
                "Model",
                "a:x",
                vec![acp::SessionConfigSelectOption::new("a:x", "A")],
            )
            .meta(meta),
        ];

        let mut screen = App::new("test-agent".to_string(), &opts, vec![]);
        screen.open_config_overlay();

        let overlay = screen.config_overlay.as_mut().expect("overlay should open");
        // Press Enter on the model entry (first and only entry)
        overlay.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // Model selector footer should show Toggle, not regular picker's Confirm
        let context = RenderContext::new((80, 24));
        let lines = overlay.render(&context);
        let footer = lines[lines.len() - 2].plain_text();
        assert!(
            footer.contains("Toggle"),
            "expected model selector (Toggle footer), got: {footer}"
        );
    }

    #[test]
    fn extract_mode_display_single_value() {
        let opts = vec![
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
        ];

        let display = extract_mode_display(&opts).expect("should extract mode display");
        assert_eq!(display, "Planner");
    }

    #[test]
    fn extract_mode_display_prefers_mode_id_over_category() {
        let opts = vec![
            SessionConfigOption::select(
                "not-mode",
                "Not Mode",
                "alt",
                vec![acp::SessionConfigSelectOption::new("alt", "Alt")],
            )
            .category(SessionConfigOptionCategory::Mode),
            SessionConfigOption::select(
                "mode",
                "Mode",
                "planner",
                vec![
                    acp::SessionConfigSelectOption::new("planner", "Planner"),
                    acp::SessionConfigSelectOption::new("coder", "Coder"),
                ],
            ),
        ];

        let display = extract_mode_display(&opts).expect("should extract mode display");
        assert_eq!(display, "Planner");
    }

    #[test]
    fn extract_mode_display_grouped_options() {
        let opts = vec![SessionConfigOption::new(
            "mode",
            "Mode",
            SessionConfigKind::Select(acp::SessionConfigSelect::new(
                "planner",
                vec![acp::SessionConfigSelectGroup::new(
                    "default",
                    "Default",
                    vec![acp::SessionConfigSelectOption::new("planner", "Planner")],
                )],
            )),
        )];

        let display = extract_mode_display(&opts).expect("should extract mode display");
        assert_eq!(display, "Planner");
    }

    #[test]
    fn extract_mode_display_returns_none_when_no_mode_option() {
        let opts = vec![SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![acp::SessionConfigSelectOption::new("m1", "M1")],
        )];

        assert!(
            extract_mode_display(&opts).is_none(),
            "should return None when no mode option exists"
        );
    }

    #[test]
    fn config_option_update_refreshes_mode_display() {
        let initial_opts = vec![
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
        ];

        let mut app = App::new("test-agent".to_string(), &initial_opts, vec![]);
        assert_eq!(app.mode_display, Some("Planner".to_string()));

        // Simulate config update changing mode to coder
        let updated_opts = vec![
            SessionConfigOption::select(
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

        let update =
            SessionUpdate::ConfigOptionUpdate(acp::ConfigOptionUpdate::new(updated_opts.clone()));
        app.on_session_update(update);

        assert_eq!(app.mode_display, Some("Coder".to_string()));
    }

    fn reasoning_options(current: &'static str) -> Vec<SessionConfigOption> {
        vec![SessionConfigOption::select(
            "reasoning_effort",
            "Reasoning Effort",
            current,
            vec![
                acp::SessionConfigSelectOption::new("none", "None"),
                acp::SessionConfigSelectOption::new("low", "Low"),
                acp::SessionConfigSelectOption::new("medium", "Medium"),
                acp::SessionConfigSelectOption::new("high", "High"),
            ],
        )]
    }

    fn assert_cycles_reasoning(from: &'static str, to: &'static str) {
        let options = reasoning_options(from);
        let mut app = App::new("test-agent".to_string(), &options, vec![]);
        let effects = app.on_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));

        assert!(effects.iter().any(|event| {
            matches!(
                event,
                AppEvent::SetConfigOption { config_id, new_value }
                if config_id == "reasoning_effort" && new_value == to
            )
        }));
    }

    #[test]
    fn meta_t_cycles_reasoning_none_to_low() {
        assert_cycles_reasoning("none", "low");
    }

    #[test]
    fn meta_t_cycles_reasoning_low_to_medium() {
        assert_cycles_reasoning("low", "medium");
    }

    #[test]
    fn meta_t_cycles_reasoning_medium_to_high() {
        assert_cycles_reasoning("medium", "high");
    }

    #[test]
    fn meta_t_cycles_reasoning_high_to_none() {
        assert_cycles_reasoning("high", "none");
    }

    #[test]
    fn meta_t_emits_render_event() {
        let options = reasoning_options("none");
        let mut app = App::new("test-agent".to_string(), &options, vec![]);
        let effects = app.on_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));

        assert!(
            effects
                .iter()
                .any(|event| matches!(event, AppEvent::Render))
        );
    }

    #[test]
    fn meta_t_ignored_when_overlay_consumes_input() {
        let options = reasoning_options("medium");
        let mut app = App::new("test-agent".to_string(), &options, vec![]);
        app.open_config_overlay();

        let effects = app.on_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));

        assert!(
            !effects
                .iter()
                .any(|event| matches!(event, AppEvent::SetConfigOption { .. })),
            "overlay should consume meta+t"
        );
    }

    #[test]
    fn meta_t_noop_when_reasoning_option_absent() {
        let options = vec![SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![acp::SessionConfigSelectOption::new("m1", "M1")],
        )];

        let mut app = App::new("test-agent".to_string(), &options, vec![]);
        let effects = app.on_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));

        assert!(
            effects.is_empty(),
            "meta+t should be no-op when reasoning option absent"
        );
    }

    #[test]
    fn extract_reasoning_effort_returns_high() {
        let opts = reasoning_options("high");
        assert_eq!(extract_reasoning_effort(&opts), Some(ReasoningEffort::High));
    }

    #[test]
    fn extract_reasoning_effort_returns_none_for_none_value() {
        let opts = reasoning_options("none");
        assert_eq!(extract_reasoning_effort(&opts), None);
    }

    #[test]
    fn extract_reasoning_effort_returns_none_when_missing() {
        let opts = vec![SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![acp::SessionConfigSelectOption::new("m1", "M1")],
        )];

        assert_eq!(extract_reasoning_effort(&opts), None);
    }
}
