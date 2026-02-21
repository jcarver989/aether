use crate::components::command_picker::{CommandEntry, CommandPicker, CommandPickerAction};
use crate::components::config_menu::{ConfigChange, ConfigMenu, ConfigMenuAction};
use crate::components::config_picker::{ConfigPicker, ConfigPickerAction};
use crate::components::container::Container;
#[cfg(test)]
use crate::components::conversation_window::SegmentContent;
use crate::components::conversation_window::{ConversationBuffer, ConversationWindow};
use crate::components::file_picker::{FileMatch, FilePicker, FilePickerAction};
use crate::components::input_prompt::InputPrompt;
use crate::components::status_line::StatusLine;
use crate::components::text_input::{TextInput, TextInputAction};
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::tui::spinner::Spinner;
use crate::tui::{
    Cursor, CursorComponent, HandlesInput, InputOutcome, Line, RenderContext, RenderOutput,
};
use acp_utils::notifications::{
    CONTEXT_USAGE_METHOD, SUB_AGENT_PROGRESS_METHOD, SubAgentProgressParams,
};
use agent_client_protocol::{
    self as acp, ExtNotification, SessionConfigKind, SessionConfigOption,
    SessionConfigSelectOptions, SessionUpdate,
};
use crossterm::event::{self, KeyCode, KeyEvent};
use std::collections::HashSet;
use std::path::Path;
use unicode_width::UnicodeWidthStr;
use url::Url;

const MAX_EMBED_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub enum AppEvent {
    Exit,
    Render,
    PushScrollback(Vec<Line>),
    PromptSubmit {
        user_input: String,
        content_blocks: Option<Vec<acp::ContentBlock>>,
    },
    SetConfigOption {
        config_id: String,
        new_value: String,
    },
    Cancel,
}

pub struct App {
    tool_call_statuses: ToolCallStatuses,
    grid_loader: Spinner,
    conversation: ConversationBuffer,
    pub(crate) text_input: TextInput,
    agent_name: String,
    model_display: Option<String>,
    config_options: Vec<SessionConfigOption>,
    waiting_for_response: bool,
    animation_tick: u16,
    available_commands: Vec<CommandEntry>,
    context_usage_pct: Option<u8>,
    file_picker: Option<FilePicker>,
    command_picker: Option<CommandPicker>,
    config_menu: Option<ConfigMenu>,
    config_picker: Option<ConfigPicker>,
}

impl App {
    pub fn new(agent_name: String, config_options: &[SessionConfigOption]) -> Self {
        Self {
            tool_call_statuses: ToolCallStatuses::new(),
            grid_loader: Spinner::default(),
            conversation: ConversationBuffer::new(),
            text_input: TextInput::new(),
            agent_name,
            model_display: extract_model_display(config_options),
            config_options: config_options.to_vec(),
            waiting_for_response: false,
            animation_tick: 0,
            available_commands: Vec::new(),
            context_usage_pct: None,
            file_picker: None,
            command_picker: None,
            config_menu: None,
            config_picker: None,
        }
    }

    pub fn on_key_event(&mut self, key_event: KeyEvent) -> Vec<AppEvent> {
        if key_event.code == KeyCode::Char('c')
            && key_event.modifiers.contains(event::KeyModifiers::CONTROL)
        {
            return vec![AppEvent::Exit];
        }

        if let Some(effects) = self.handle_picker_key(key_event) {
            return effects;
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
                self.model_display = extract_model_display(&update.config_options);
                self.config_options.clone_from(&update.config_options);
                self.update_config_menu(&update.config_options);
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

    pub fn on_prompt_done(&mut self, render_size: (u16, u16)) -> Vec<AppEvent> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        self.conversation.close_thought_block();

        let context = RenderContext::new(render_size);
        let (scrollback_lines, completed_tool_ids) = self
            .conversation
            .flush_completed(&self.tool_call_statuses, &context);

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
        if self.waiting_for_response || self.tool_call_statuses.has_running() {
            self.animation_tick = self.animation_tick.wrapping_add(1);
            self.grid_loader.tick = self.animation_tick;
            self.tool_call_statuses.set_tick(self.animation_tick);
            vec![AppEvent::Render]
        } else {
            vec![]
        }
    }

    pub fn on_ext_notification(&mut self, notification: &ExtNotification) -> Vec<AppEvent> {
        match notification.method.as_ref() {
            CONTEXT_USAGE_METHOD => {
                if let Some(ratio) =
                    serde_json::from_str::<serde_json::Value>(notification.params.get())
                        .ok()
                        .and_then(|v| v.get("usage_ratio")?.as_f64())
                {
                    // Safety: clamp guarantees value is in [0.0, 100.0], round() keeps it integral
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let pct_left = ((1.0 - ratio) * 100.0).clamp(0.0, 100.0).round() as u8;
                    self.context_usage_pct = Some(pct_left);
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
            _ => {}
        }
        vec![]
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
    pub fn has_config_menu(&self) -> bool {
        self.config_menu.is_some()
    }

    #[allow(dead_code)]
    pub fn has_config_picker(&self) -> bool {
        self.config_picker.is_some()
    }

    #[allow(dead_code)]
    pub fn config_menu_selected_index(&self) -> Option<usize> {
        self.config_menu.as_ref().map(|m| m.selected_index)
    }

    #[allow(dead_code)]
    pub fn config_picker_config_id(&self) -> Option<&str> {
        self.config_picker.as_ref().map(|p| p.config_id.as_str())
    }

    #[allow(dead_code)]
    pub fn file_picker_selected_display_name(&self) -> Option<String> {
        self.file_picker
            .as_ref()
            .and_then(|p| p.combobox.selected().map(|f| f.display_name.clone()))
    }

    #[allow(dead_code)]
    pub fn command_picker_match_names(&self) -> Vec<&str> {
        self.command_picker
            .as_ref()
            .map(|p| p.combobox.matches.iter().map(|m| m.name.as_str()).collect())
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

        if let Some(ref mut picker) = self.config_picker {
            let outcome = picker.handle_key(key_event);
            return Some(self.handle_config_picker_outcome(outcome));
        }

        if let Some(ref mut menu) = self.config_menu {
            let outcome = menu.handle_key(key_event);
            return Some(self.handle_config_menu_outcome(outcome));
        }

        None
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
                let selected = self
                    .file_picker
                    .take()
                    .and_then(|p| p.combobox.selected().cloned());
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
            Some(CommandPickerAction::CloseAndClearInput) => {
                self.command_picker = None;
                self.text_input.clear();
                if outcome.needs_render {
                    vec![AppEvent::Render]
                } else {
                    vec![]
                }
            }
            Some(CommandPickerAction::CommandChosen(ref cmd)) => {
                self.command_picker = None;
                self.apply_command(cmd)
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

    fn handle_config_picker_outcome(
        &mut self,
        outcome: InputOutcome<ConfigPickerAction>,
    ) -> Vec<AppEvent> {
        match outcome.action {
            Some(ConfigPickerAction::Close) => {
                self.config_picker = None;
                if outcome.needs_render {
                    vec![AppEvent::Render]
                } else {
                    vec![]
                }
            }
            Some(ConfigPickerAction::ApplySelection(change)) => {
                self.config_picker = None;
                if let Some(change) = change {
                    Self::handle_config_change(change)
                } else if outcome.needs_render {
                    vec![AppEvent::Render]
                } else {
                    vec![]
                }
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

    fn handle_config_menu_outcome(
        &mut self,
        outcome: InputOutcome<ConfigMenuAction>,
    ) -> Vec<AppEvent> {
        if let Some(action) = outcome.action {
            match action {
                ConfigMenuAction::CloseAll => {
                    self.config_menu = None;
                    self.config_picker = None;
                }
                ConfigMenuAction::OpenSelectedPicker => {
                    self.config_picker = self
                        .config_menu
                        .as_ref()
                        .and_then(|menu| menu.selected_entry())
                        .and_then(ConfigPicker::from_entry);
                }
            }
        }

        if outcome.needs_render {
            vec![AppEvent::Render]
        } else {
            vec![]
        }
    }

    fn handle_config_change(change: ConfigChange) -> Vec<AppEvent> {
        vec![
            AppEvent::SetConfigOption {
                config_id: change.config_id,
                new_value: change.new_value,
            },
            AppEvent::Render,
        ]
    }

    fn apply_command(&mut self, cmd: &CommandEntry) -> Vec<AppEvent> {
        if cmd.builtin && cmd.name == "config" {
            self.text_input.clear();
            self.close_all_pickers();
            let options = self.config_options.clone();
            self.open_config_menu(&options);
            self.config_picker = self
                .config_menu
                .as_ref()
                .filter(|menu| menu.options.len() == 1)
                .and_then(|menu| menu.options.first())
                .and_then(ConfigPicker::from_entry);
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
        self.text_input.clear();
        self.close_input_pickers();

        let mut effects = vec![
            AppEvent::PushScrollback(vec![Line::new(String::new())]),
            AppEvent::PushScrollback(vec![Line::new(user_input.clone())]),
        ];

        let (content_blocks, warning_lines) = self.build_attachment_blocks(&user_input);
        if !warning_lines.is_empty() {
            effects.push(AppEvent::PushScrollback(warning_lines));
        }

        effects.push(AppEvent::PromptSubmit {
            user_input,
            content_blocks: if content_blocks.is_empty() {
                None
            } else {
                Some(content_blocks)
            },
        });

        self.waiting_for_response = true;
        self.animation_tick = 0;
        self.grid_loader.visible = true;
        self.grid_loader.tick = 0;

        effects.push(AppEvent::Render);
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

    fn open_config_menu(&mut self, options: &[SessionConfigOption]) {
        self.config_menu = Some(ConfigMenu::from_config_options(options));
    }

    #[cfg(test)]
    fn open_config_picker_for(&mut self, config_id: &str) -> bool {
        let Some(menu) = self.config_menu.as_ref() else {
            return false;
        };
        let Some(entry) = menu.entry_by_id(config_id) else {
            return false;
        };
        let Some(picker) = ConfigPicker::from_entry(entry) else {
            return false;
        };
        self.config_picker = Some(picker);
        true
    }

    fn close_all_pickers(&mut self) {
        self.file_picker = None;
        self.command_picker = None;
        self.config_picker = None;
    }

    fn close_input_pickers(&mut self) {
        self.file_picker = None;
        self.command_picker = None;
    }

    fn update_config_menu(&mut self, options: &[SessionConfigOption]) {
        if let Some(ref mut menu) = self.config_menu {
            menu.update_options(options);
        }
    }

    fn config_picker_cursor_col(picker: &ConfigPicker) -> usize {
        let prefix = format!("  {} search: ", picker.title);
        UnicodeWidthStr::width(prefix.as_str())
            + UnicodeWidthStr::width(picker.combobox.query.as_str())
    }

    fn command_picker_cursor_col(picker: &CommandPicker) -> usize {
        let prefix = "  / search: ";
        UnicodeWidthStr::width(prefix) + UnicodeWidthStr::width(picker.combobox.query.as_str())
    }

    fn build_attachment_blocks(&mut self, user_input: &str) -> (Vec<acp::ContentBlock>, Vec<Line>) {
        let mentions: HashSet<&str> = user_input.split_whitespace().collect();
        let selected = self.text_input.take_mentions();
        let mut warning_lines = Vec::new();
        let mut blocks = Vec::new();

        for mention in selected {
            if !mentions.contains(mention.mention.as_str()) {
                continue;
            }

            match try_build_attachment_block(&mention.path, &mention.display_name) {
                Ok((block, maybe_warning)) => {
                    blocks.push(block);
                    if let Some(warning) = maybe_warning {
                        warning_lines.push(Line::new(format!("[wisp] {warning}")));
                    }
                }
                Err(warning) => warning_lines.push(Line::new(format!("[wisp] {warning}"))),
            }
        }

        (blocks, warning_lines)
    }
}

fn try_build_attachment_block(
    path: &Path,
    display_name: &str,
) -> Result<(acp::ContentBlock, Option<String>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read {display_name}: {e}"))?;

    let truncated = bytes.len() > MAX_EMBED_TEXT_BYTES;
    let text_bytes = if truncated {
        &bytes[..MAX_EMBED_TEXT_BYTES]
    } else {
        &bytes
    };

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

    let file_uri =
        Url::from_file_path(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
            .map_err(|()| format!("Failed to build file URI for {display_name}"))?
            .to_string();

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

impl CursorComponent for App {
    fn render_with_cursor(&mut self, context: &RenderContext) -> RenderOutput {
        // Compute values before borrowing fields mutably into the container.
        let command_picker_col = self
            .command_picker
            .as_ref()
            .map(Self::command_picker_cursor_col);
        let config_picker_col = self
            .config_picker
            .as_ref()
            .map(Self::config_picker_cursor_col);
        let picker_query_len = self.file_picker.as_ref().map(|p| p.combobox.query.len());
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
        let mut status_line = StatusLine {
            agent_name: &self.agent_name,
            model_display: self.model_display.as_deref(),
            context_pct_left: self.context_usage_pct,
            waiting_for_response: self.waiting_for_response,
        };

        let mut container: Container<'_> =
            Container::new(vec![&mut conversation_window, &mut input_prompt]);
        let input_component_index = 1;

        if let Some(ref mut picker) = self.file_picker {
            container.push(picker);
        }

        let command_picker_index = if let Some(ref mut picker) = self.command_picker {
            let idx = container.len();
            container.push(picker);
            Some(idx)
        } else {
            None
        };

        let config_picker_index = if let Some(ref mut picker) = self.config_picker {
            let idx = container.len();
            container.push(picker);
            Some(idx)
        } else {
            if let Some(ref mut menu) = self.config_menu {
                container.push(menu);
            }
            None
        };

        container.push(&mut status_line);
        let (lines, offsets) = container.render_with_offsets(context);

        let mut cursor = Cursor {
            logical_row: offsets[input_component_index] + input_layout.cursor_row,
            col: input_layout.cursor_col as usize,
        };

        if let Some(idx) = command_picker_index {
            cursor = Cursor {
                logical_row: offsets[idx],
                col: command_picker_col.unwrap_or(0),
            };
        }

        if let Some(idx) = config_picker_index {
            cursor = Cursor {
                logical_row: offsets[idx],
                col: config_picker_col.unwrap_or(0),
            };
        }

        RenderOutput { lines, cursor }
    }
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

fn extract_model_display(config_options: &[SessionConfigOption]) -> Option<String> {
    let option = config_options.iter().find(|o| o.id.0.as_ref() == "model")?;

    let SessionConfigKind::Select(ref select) = option.kind else {
        return None;
    };

    let SessionConfigSelectOptions::Ungrouped(ref options) = select.options else {
        return None;
    };

    options
        .iter()
        .find(|o| o.value == select.current_value)
        .map(|o| o.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use std::path::PathBuf;

    #[test]
    fn command_picker_cursor_targets_picker_header() {
        let mut screen = App::new("test-agent".to_string(), &[]);
        screen.command_picker = Some(CommandPicker::new(vec![CommandEntry {
            name: "config".to_string(),
            description: "Open config".to_string(),
            has_input: false,
            hint: None,
            builtin: true,
        }]));

        let context = RenderContext::new((120, 40));
        let output = screen.render_with_cursor(&context);

        let row = output
            .lines
            .iter()
            .position(|line| line.plain_text().contains("  / search: "))
            .expect("command picker header should exist");
        assert_eq!(output.cursor.logical_row, row);
    }

    #[test]
    fn config_picker_takes_precedence_over_config_menu() {
        let mut screen = App::new("test-agent".to_string(), &[]);
        let opts = vec![agent_client_protocol::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![agent_client_protocol::SessionConfigSelectOption::new(
                "m1", "M1",
            )],
        )];
        screen.open_config_menu(&opts);
        screen.open_config_picker_for("model");

        let context = RenderContext::new((120, 40));
        let output = screen.render_with_cursor(&context);

        let has_menu_row = output
            .lines
            .iter()
            .any(|line| line.plain_text().contains("Model: M1"));
        assert!(
            !has_menu_row,
            "config menu should be hidden when picker is open"
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
        let mut screen = App::new("test-agent".to_string(), &options);
        let effects = screen.apply_command(&CommandEntry {
            name: "config".to_string(),
            description: "Open configuration settings".to_string(),
            has_input: false,
            hint: None,
            builtin: true,
        });

        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
        assert!(screen.config_menu.is_some());
        assert_eq!(screen.text_input.buffer(), "");
    }

    #[test]
    fn command_without_input_submits_prompt_immediately() {
        let mut screen = App::new("test-agent".to_string(), &[]);
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
        let mut screen = App::new("test-agent".to_string(), &[]);
        screen.text_input.set_input("@fo".to_string());

        screen
            .text_input
            .apply_file_selection(PathBuf::from("foo.rs"), "foo.rs".to_string());

        assert_eq!(screen.text_input.buffer(), "@foo.rs ");
    }

    #[test]
    fn config_with_single_option_opens_picker_directly() {
        let options = vec![agent_client_protocol::SessionConfigOption::select(
            "model",
            "Model",
            "m1",
            vec![
                agent_client_protocol::SessionConfigSelectOption::new("m1", "M1"),
                agent_client_protocol::SessionConfigSelectOption::new("m2", "M2"),
            ],
        )];
        let mut screen = App::new("test-agent".to_string(), &options);
        let effects = screen.apply_command(&CommandEntry {
            name: "config".to_string(),
            description: "Open configuration settings".to_string(),
            has_input: false,
            hint: None,
            builtin: true,
        });

        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
        assert!(screen.config_menu.is_some());
        assert!(
            screen.config_picker.is_some(),
            "Single config option should auto-open picker"
        );
        assert_eq!(screen.config_picker_config_id(), Some("model"));
    }

    #[test]
    fn prompt_done_keeps_running_tool_segment() {
        let mut screen = App::new("test-agent".to_string(), &[]);

        let tool_call = acp::ToolCall::new("tool-1", "Read file");
        screen.tool_call_statuses.on_tool_call(&tool_call);
        screen.conversation.ensure_tool_segment("tool-1");

        let effects = screen.on_prompt_done((120, 40));

        assert!(matches!(effects.as_slice(), [AppEvent::Render]));
        let segments: Vec<_> = screen.conversation.segments().collect();
        assert!(matches!(segments[..], [SegmentContent::ToolCall(id)] if id == "tool-1"));
    }

    #[test]
    fn ctrl_c_exits() {
        let mut screen = App::new("test-agent".to_string(), &[]);

        let effects = screen.on_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

        assert!(matches!(effects.as_slice(), [AppEvent::Exit]));
    }

    #[test]
    fn escape_while_waiting_emits_cancel() {
        let mut screen = App::new("test-agent".to_string(), &[]);
        screen.waiting_for_response = true;

        let effects = screen.on_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(matches!(effects.as_slice(), [AppEvent::Cancel]));
    }

    #[test]
    fn streaming_chunks_keep_waiting_for_response() {
        let mut screen = App::new("test-agent".to_string(), &[]);
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
        let mut screen = App::new("test-agent".to_string(), &[]);
        screen.waiting_for_response = false;

        let effects = screen.on_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(effects.is_empty());
    }

    #[test]
    fn sub_agent_progress_notification_triggers_render() {
        let mut app = App::new("test-agent".to_string(), &[]);
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
        let mut app = App::new("test-agent".to_string(), &[]);
        let raw = serde_json::value::to_raw_value(&serde_json::json!({"bad": "data"})).unwrap();
        let notification =
            acp::ExtNotification::new("_aether/sub_agent_progress", std::sync::Arc::from(raw));

        let effects = app.on_ext_notification(&notification);
        assert!(effects.is_empty());
    }

    #[test]
    fn paste_inserts_at_cursor_position() {
        let mut app = App::new("test".to_string(), &[]);
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
        let mut app = App::new("test".to_string(), &[]);
        app.text_input.set_input("hello".to_string());

        app.execute_input();

        assert_eq!(app.text_input.cursor_index(None), 0);
        assert!(app.text_input.buffer().is_empty());
    }
}
