use crate::components::command_picker::{CommandEntry, CommandPicker};
use crate::components::config_menu::ConfigMenu;
use crate::components::config_picker::ConfigPicker;
use crate::components::file_picker::FilePicker;
use crate::components::grid_loader::GridLoader;
use crate::components::input_prompt::InputPrompt;
use crate::components::status_line::StatusLine;
use crate::components::thought_message::ThoughtMessage;
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::error::WispError;
use crate::tui::soft_wrap::soft_wrap_lines_with_map;
use crate::tui::{Component, FrameRenderer, Line, RenderContext, Screen};
use acp_utils::client::AcpPromptHandle;
use agent_client_protocol::{
    self as acp, ExtNotification, SessionConfigKind, SessionConfigOption,
    SessionConfigSelectOptions, SessionUpdate,
};
use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;
use url::Url;

const MAX_EMBED_TEXT_BYTES: usize = 1024 * 1024;

pub enum LoopAction {
    Continue,
    Exit,
}

#[derive(Debug, Clone)]
struct SelectedFileMention {
    mention: String,
    path: PathBuf,
    display_name: String,
}

struct StreamingMessage<'a> {
    text: &'a str,
}

impl Component for StreamingMessage<'_> {
    fn render(&self, _context: &RenderContext) -> Vec<Line> {
        if self.text.is_empty() {
            return vec![];
        }

        self.text
            .lines()
            .map(|line| Line::new(line.to_string()))
            .collect()
    }
}

enum StreamSegment {
    Text(String),
    Thought(String),
    ToolCall(String),
}

struct FrameSnapshot {
    visual_lines: Vec<Line>,
    cursor_row: usize,
    cursor_col: u16,
}

pub struct Renderer<T: Write> {
    tui: FrameRenderer<T>,
    tool_call_statuses: ToolCallStatuses,
    grid_loader: GridLoader,
    stream_segments: Vec<StreamSegment>,
    thought_block_open: bool,
    input_buffer: String,
    agent_name: String,
    model_display: Option<String>,
    config_options: Vec<SessionConfigOption>,
    waiting_for_response: bool,
    animation_tick: u16,
    pub file_picker: Option<FilePicker>,
    pub command_picker: Option<CommandPicker>,
    pub config_menu: Option<ConfigMenu>,
    pub config_picker: Option<ConfigPicker>,
    pub available_commands: Vec<CommandEntry>,
    pending_open_model_picker: bool,
    selected_mentions: Vec<SelectedFileMention>,
    context_usage_pct: Option<u8>,
}

impl<T: Write> Renderer<T> {
    pub fn new(writer: T, agent_name: String, config_options: &[SessionConfigOption]) -> Self {
        Self {
            tui: FrameRenderer::new(writer),
            tool_call_statuses: ToolCallStatuses::new(),
            grid_loader: GridLoader::new(),
            stream_segments: Vec::new(),
            thought_block_open: false,
            input_buffer: String::new(),
            agent_name,
            model_display: extract_model_display(config_options),
            config_options: config_options.to_vec(),
            waiting_for_response: false,
            animation_tick: 0,
            file_picker: None,
            command_picker: None,
            config_menu: None,
            config_picker: None,
            available_commands: Vec::new(),
            pending_open_model_picker: false,
            selected_mentions: Vec::new(),
            context_usage_pct: None,
        }
    }

    /// Get a reference to the writer (useful for testing)
    #[allow(dead_code)]
    pub fn writer(&self) -> &T {
        self.tui.writer()
    }

    /// Get a mutable reference to the writer (useful for testing)
    #[allow(dead_code)]
    pub fn writer_mut(&mut self) -> &mut T {
        self.tui.writer_mut()
    }

    /// Render the current frame to the terminal.
    fn render_frame(&mut self) -> std::io::Result<()> {
        let snapshot = self.build_frame_snapshot();
        self.tui.render_lines(&snapshot.visual_lines)?;
        let rows_up = snapshot
            .visual_lines
            .len()
            .saturating_sub(1)
            .saturating_sub(snapshot.cursor_row) as u16;
        self.tui.reposition_cursor(rows_up, snapshot.cursor_col)?;
        Ok(())
    }

    fn build_frame_snapshot(&self) -> FrameSnapshot {
        let context = self.tui.context();
        let mut logical_lines: Vec<Line> = Vec::new();
        logical_lines.extend(self.grid_loader.render(context));
        let mut last_segment_kind: Option<std::mem::Discriminant<StreamSegment>> = None;

        for segment in &self.stream_segments {
            let kind = std::mem::discriminant(segment);
            let segment_lines = self.render_stream_segment(segment, context);
            if segment_lines.is_empty() {
                continue;
            }

            if let Some(prev_kind) = last_segment_kind
                && prev_kind != kind
            {
                logical_lines.push(Line::new(String::new()));
            }

            logical_lines.extend(segment_lines);
            last_segment_kind = Some(kind);
        }

        let input_prompt = InputPrompt {
            input: &self.input_buffer,
            cursor_index: self.input_cursor_index(),
        };
        let input_layout = input_prompt.layout(context);
        let mut cursor_logical_row = logical_lines.len() + input_layout.cursor_row;
        let mut cursor_col = input_layout.cursor_col as usize;
        logical_lines.extend(input_layout.lines);

        if let Some(ref picker) = self.file_picker {
            logical_lines.extend(picker.render(context));
        }

        if let Some(ref picker) = self.command_picker {
            cursor_logical_row = logical_lines.len();
            cursor_col = Self::command_picker_cursor_col(picker);
            logical_lines.extend(picker.render(context));
        }

        if let Some(ref picker) = self.config_picker {
            cursor_logical_row = logical_lines.len();
            cursor_col = Self::config_picker_cursor_col(picker);
            logical_lines.extend(picker.render(context));
        } else if let Some(ref menu) = self.config_menu {
            logical_lines.extend(menu.render(context));
        }

        let status_line = StatusLine {
            agent_name: &self.agent_name,
            model_display: self.model_display.as_deref(),
            context_pct_left: self.context_usage_pct,
        };
        logical_lines.extend(status_line.render(context));

        let (visual_lines, logical_to_visual) =
            soft_wrap_lines_with_map(&logical_lines, context.size.0);

        let mut cursor_row = logical_to_visual
            .get(cursor_logical_row)
            .copied()
            .unwrap_or_else(|| visual_lines.len().saturating_sub(1));
        let width = context.size.0 as usize;
        if width > 0 {
            cursor_row += cursor_col / width;
            cursor_col %= width;
        } else {
            cursor_col = 0;
        }
        if cursor_row >= visual_lines.len() {
            cursor_row = visual_lines.len().saturating_sub(1);
        }

        FrameSnapshot {
            visual_lines,
            cursor_row,
            cursor_col: cursor_col as u16,
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

    fn input_cursor_index(&self) -> usize {
        if let Some(ref picker) = self.file_picker {
            let at_pos = self
                .active_mention_start()
                .unwrap_or_else(|| self.input_buffer.len());
            at_pos + 1 + picker.combobox.query.len()
        } else {
            self.input_buffer.len()
        }
    }

    pub fn on_key_event(
        &mut self,
        key_event: KeyEvent,
        prompt_handle: &AcpPromptHandle,
        session_id: &acp::SessionId,
    ) -> Result<LoopAction, WispError> {
        if key_event.code == KeyCode::Char('c')
            && key_event.modifiers.contains(event::KeyModifiers::CONTROL)
        {
            return Ok(LoopAction::Exit);
        }

        if let Some(ref mut picker) = self.file_picker {
            match key_event.code {
                KeyCode::Esc => {
                    self.file_picker = None;
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Up => {
                    picker.combobox.move_selection_up();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Char('p') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    picker.combobox.move_selection_up();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Down => {
                    picker.combobox.move_selection_down();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Char('n') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    picker.combobox.move_selection_down();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Enter => {
                    self.confirm_file_selection()?;
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Char(c) => {
                    if c.is_whitespace() {
                        self.input_buffer.push(c);
                        self.file_picker = None;
                        self.render_frame()?;
                        return Ok(LoopAction::Continue);
                    }

                    self.input_buffer.push(c);
                    let query = if let Some(at_pos) = Self::mention_start(&self.input_buffer) {
                        self.input_buffer[at_pos + 1..].to_string()
                    } else {
                        String::new()
                    };
                    picker.combobox.update_query(query);
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Backspace => {
                    if !self.input_buffer.is_empty() {
                        let last = self.input_buffer.pop();
                        if last == Some('@') {
                            self.file_picker = None;
                        } else if let Some(at_pos) = Self::mention_start(&self.input_buffer) {
                            let query = self.input_buffer[at_pos + 1..].to_string();
                            picker.combobox.update_query(query);
                        } else {
                            self.file_picker = None;
                        }
                        self.render_frame()?;
                    }
                    return Ok(LoopAction::Continue);
                }
                _ => {}
            }
        }

        if self.command_picker.is_some() {
            match key_event.code {
                KeyCode::Esc => {
                    self.command_picker = None;
                    self.input_buffer.clear();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Up => {
                    self.command_picker
                        .as_mut()
                        .unwrap()
                        .combobox
                        .move_selection_up();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Char('p') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.command_picker
                        .as_mut()
                        .unwrap()
                        .combobox
                        .move_selection_up();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Down => {
                    self.command_picker
                        .as_mut()
                        .unwrap()
                        .combobox
                        .move_selection_down();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Char('n') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.command_picker
                        .as_mut()
                        .unwrap()
                        .combobox
                        .move_selection_down();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Enter => {
                    if let Some(cmd) = self
                        .command_picker
                        .as_ref()
                        .and_then(|p| p.selected_command().cloned())
                    {
                        self.command_picker = None;
                        if cmd.builtin && cmd.name == "config" {
                            self.input_buffer.clear();
                            self.file_picker = None;
                            self.config_picker = None;
                            self.pending_open_model_picker = false;
                            self.config_menu =
                                Some(ConfigMenu::from_config_options(&self.config_options));
                            self.render_frame()?;
                        } else if cmd.has_input {
                            self.input_buffer = format!("/{} ", cmd.name);
                            self.render_frame()?;
                        } else {
                            self.input_buffer = format!("/{}", cmd.name);
                            return self.execute_input(prompt_handle, session_id);
                        }
                    } else {
                        self.command_picker = None;
                        self.input_buffer.clear();
                        self.render_frame()?;
                    }
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Char(c) => {
                    if !c.is_control() {
                        self.command_picker
                            .as_mut()
                            .unwrap()
                            .combobox
                            .push_query_char(c);
                        self.render_frame()?;
                    }
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Backspace => {
                    let picker = self.command_picker.as_ref().unwrap();
                    if picker.combobox.query.is_empty() {
                        self.command_picker = None;
                        self.input_buffer.clear();
                        self.render_frame()?;
                    } else {
                        self.command_picker
                            .as_mut()
                            .unwrap()
                            .combobox
                            .pop_query_char();
                        self.render_frame()?;
                    }
                    return Ok(LoopAction::Continue);
                }
                _ => return Ok(LoopAction::Continue),
            }
        }

        if let Some(ref mut picker) = self.config_picker {
            match key_event.code {
                KeyCode::Esc => {
                    self.config_picker = None;
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Up => {
                    picker.move_selection_up();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Char('p') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    picker.move_selection_up();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Down => {
                    picker.move_selection_down();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Char('n') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    picker.move_selection_down();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Enter => {
                    let confirmed_change = picker.confirm_selection();
                    let was_provider = picker.config_id == "provider";
                    self.config_picker = None;
                    if let Some(change) = confirmed_change {
                        let _ = prompt_handle.set_config_option(
                            session_id,
                            &change.config_id,
                            &change.new_value,
                        );
                        if was_provider {
                            self.pending_open_model_picker = true;
                        }
                    }
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Char(c) => {
                    if !c.is_control() {
                        picker.push_query_char(c);
                        self.render_frame()?;
                    }
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Backspace => {
                    if !picker.combobox.query.is_empty() {
                        picker.pop_query_char();
                        self.render_frame()?;
                    }
                    return Ok(LoopAction::Continue);
                }
                _ => return Ok(LoopAction::Continue),
            }
        }

        if let Some(ref mut menu) = self.config_menu {
            match key_event.code {
                KeyCode::Esc => {
                    self.config_menu = None;
                    self.config_picker = None;
                    self.pending_open_model_picker = false;
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Up => {
                    menu.move_selection_up();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Down => {
                    menu.move_selection_down();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Enter => {
                    self.config_picker = menu.selected_entry().and_then(ConfigPicker::from_entry);
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                _ => {
                    // Swallow all other keys while config menu is open
                    return Ok(LoopAction::Continue);
                }
            }
        }

        match key_event.code {
            KeyCode::Char('/') if self.input_buffer.is_empty() => {
                self.input_buffer.push('/');
                let mut commands = builtin_commands();
                commands.extend(self.available_commands.clone());
                self.command_picker = Some(CommandPicker::new(commands));
                self.render_frame()?;
            }

            KeyCode::Char('@') => {
                self.input_buffer.push('@');
                self.file_picker = Some(FilePicker::new());
                self.render_frame()?;
            }

            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                self.render_frame()?;
            }

            KeyCode::Backspace => {
                if !self.input_buffer.is_empty() {
                    self.input_buffer.pop();
                    self.render_frame()?;
                }
            }

            KeyCode::Enter => {
                return self.execute_input(prompt_handle, session_id);
            }
            _ => {}
        }
        Ok(LoopAction::Continue)
    }

    fn execute_input(
        &mut self,
        prompt_handle: &AcpPromptHandle,
        session_id: &acp::SessionId,
    ) -> Result<LoopAction, WispError> {
        if self.input_buffer.trim().is_empty() {
            self.render_frame()?;
        } else {
            let user_input = self.input_buffer.trim().to_string();
            self.input_buffer.clear();
            self.file_picker = None;
            self.command_picker = None;

            // Push the user's message line to scrollback
            self.tui
                .push_to_scrollback(&[Line::new(user_input.clone())])?;

            let content_blocks = self.build_attachment_blocks(&user_input)?;
            prompt_handle.prompt(
                session_id,
                &user_input,
                if content_blocks.is_empty() {
                    None
                } else {
                    Some(content_blocks)
                },
            )?;

            self.waiting_for_response = true;
            self.animation_tick = 0;
            self.grid_loader.visible = true;
            self.grid_loader.tick = 0;

            // Render fresh prompt
            self.render_frame()?;
        }
        Ok(LoopAction::Continue)
    }

    fn confirm_file_selection(&mut self) -> Result<(), WispError> {
        let picker = self
            .file_picker
            .take()
            .ok_or(WispError::Other("File picker not active".into()))?;

        if let Some(selected) = picker.combobox.selected() {
            let mention = format!("@{}", selected.display_name);
            self.selected_mentions.push(SelectedFileMention {
                mention: mention.clone(),
                path: selected.path.clone(),
                display_name: selected.display_name.clone(),
            });

            // Replace current @query with @filename
            if let Some(at_pos) = self.active_mention_start() {
                self.input_buffer.truncate(at_pos);
                self.input_buffer.push_str(&mention);
                self.input_buffer.push(' ');
            }
        }
        Ok(())
    }

    fn active_mention_start(&self) -> Option<usize> {
        Self::mention_start(&self.input_buffer)
    }

    fn mention_start(input: &str) -> Option<usize> {
        let at_pos = input.rfind('@')?;
        let prefix = &input[..at_pos];
        if prefix.is_empty() || prefix.chars().last().is_some_and(char::is_whitespace) {
            Some(at_pos)
        } else {
            None
        }
    }

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

    fn build_attachment_blocks(
        &mut self,
        user_input: &str,
    ) -> Result<Vec<acp::ContentBlock>, WispError> {
        let mentions: HashSet<&str> = user_input.split_whitespace().collect();
        let selected = std::mem::take(&mut self.selected_mentions);
        let mut warnings = Vec::new();
        let mut blocks = Vec::new();

        for mention in selected {
            if !mentions.contains(mention.mention.as_str()) {
                continue;
            }

            match self.try_build_attachment_block(&mention.path, &mention.display_name) {
                Ok((block, maybe_warning)) => {
                    blocks.push(block);
                    if let Some(warning) = maybe_warning {
                        warnings.push(warning);
                    }
                }
                Err(warning) => warnings.push(warning),
            }
        }

        if !warnings.is_empty() {
            let warning_lines: Vec<Line> = warnings
                .into_iter()
                .map(|warning| Line::new(format!("[wisp] {warning}")))
                .collect();
            self.tui.push_to_scrollback(&warning_lines)?;
        }

        Ok(blocks)
    }

    fn try_build_attachment_block(
        &self,
        path: &Path,
        display_name: &str,
    ) -> Result<(acp::ContentBlock, Option<String>), String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("Failed to read {display_name}: {e}"))?;

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
                .map_err(|_| format!("Failed to build file URI for {display_name}"))?
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

    /// Handle a streaming session update from ACP.
    pub fn on_session_update(&mut self, update: acp::SessionUpdate) -> std::io::Result<()> {
        let was_loading = self.grid_loader.visible;
        let mut should_render = was_loading;
        self.waiting_for_response = false;
        self.grid_loader.visible = false;

        match update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let acp::ContentBlock::Text(text_content) = chunk.content {
                    self.append_text_chunk(&text_content.text);
                    should_render = true;
                }
            }

            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let acp::ContentBlock::Text(text_content) = chunk.content {
                    self.append_thought_chunk(&text_content.text);
                    should_render = true;
                }
            }

            SessionUpdate::ToolCall(tool_call) => {
                self.close_thought_block();
                self.tool_call_statuses.on_tool_call(&tool_call);
                self.ensure_tool_segment(&tool_call.tool_call_id.0);
                should_render = true;
            }

            SessionUpdate::ToolCallUpdate(update) => {
                self.close_thought_block();
                self.tool_call_statuses.on_tool_call_update(&update);
                if self.tool_call_statuses.has_tool(&update.tool_call_id.0) {
                    self.ensure_tool_segment(&update.tool_call_id.0);
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
                self.close_thought_block();
                self.model_display = extract_model_display(&update.config_options);
                self.config_options = update.config_options.clone();
                if let Some(ref mut menu) = self.config_menu {
                    menu.update_options(&update.config_options);
                }
                if self.pending_open_model_picker {
                    self.pending_open_model_picker = false;
                    let _ = self.open_config_picker_for("model");
                }
                should_render = true;
            }

            _ => {
                self.close_thought_block();
            }
        }

        if should_render {
            self.render_frame()?;
        }

        Ok(())
    }

    /// Called when the agent's prompt response is complete.
    /// Flushes accumulated text and completed tool calls to scrollback.
    pub fn on_prompt_done(&mut self) -> std::io::Result<()> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        self.close_thought_block();

        let stream_segments = std::mem::take(&mut self.stream_segments);
        let mut remaining_segments = Vec::new();
        let context = self.tui.context();

        let mut scrollback_lines: Vec<Line> = Vec::new();
        let mut last_segment_kind: Option<std::mem::Discriminant<StreamSegment>> = None;

        for segment in stream_segments {
            let kind = std::mem::discriminant(&segment);
            match segment {
                StreamSegment::Thought(text) => {
                    let segment_lines = ThoughtMessage { text: &text }.render(context);
                    Self::extend_with_vertical_margin(
                        &mut scrollback_lines,
                        &mut last_segment_kind,
                        kind,
                        segment_lines,
                    );
                }
                StreamSegment::Text(text) => {
                    let segment_lines = text
                        .lines()
                        .map(|text_line| Line::new(text_line.to_string()))
                        .collect();
                    Self::extend_with_vertical_margin(
                        &mut scrollback_lines,
                        &mut last_segment_kind,
                        kind,
                        segment_lines,
                    );
                }
                StreamSegment::ToolCall(id) => {
                    if self.tool_call_statuses.is_tool_running(&id) {
                        remaining_segments.push(StreamSegment::ToolCall(id));
                        continue;
                    }

                    let segment_lines = self.tool_call_statuses.render_tool(&id, context);
                    Self::extend_with_vertical_margin(
                        &mut scrollback_lines,
                        &mut last_segment_kind,
                        kind,
                        segment_lines,
                    );
                    self.tool_call_statuses.remove_tool(&id);
                }
            }
        }

        self.stream_segments = remaining_segments;

        if !scrollback_lines.is_empty() {
            self.tui.push_to_scrollback(&scrollback_lines)?;
        }

        self.render_frame()?;
        Ok(())
    }

    fn append_text_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        self.close_thought_block();

        match self.stream_segments.last_mut() {
            Some(StreamSegment::Text(existing)) => existing.push_str(chunk),
            _ => self
                .stream_segments
                .push(StreamSegment::Text(chunk.to_string())),
        }
    }

    fn append_thought_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        if self.thought_block_open {
            if let Some(StreamSegment::Thought(existing)) = self.stream_segments.last_mut() {
                existing.push_str(chunk);
                return;
            }
        }

        self.stream_segments
            .push(StreamSegment::Thought(chunk.to_string()));
        self.thought_block_open = true;
    }

    fn close_thought_block(&mut self) {
        self.thought_block_open = false;
    }

    fn ensure_tool_segment(&mut self, tool_id: &str) {
        let has_segment = self
            .stream_segments
            .iter()
            .any(|segment| matches!(segment, StreamSegment::ToolCall(id) if id == tool_id));

        if !has_segment {
            self.stream_segments
                .push(StreamSegment::ToolCall(tool_id.to_string()));
        }
    }

    fn render_stream_segment(&self, segment: &StreamSegment, context: &RenderContext) -> Vec<Line> {
        match segment {
            StreamSegment::Thought(text) => ThoughtMessage { text }.render(context),
            StreamSegment::Text(text) => StreamingMessage { text }.render(context),
            StreamSegment::ToolCall(id) => self.tool_call_statuses.render_tool(id, context),
        }
    }

    fn extend_with_vertical_margin(
        target: &mut Vec<Line>,
        last_segment_kind: &mut Option<std::mem::Discriminant<StreamSegment>>,
        kind: std::mem::Discriminant<StreamSegment>,
        lines: Vec<Line>,
    ) {
        if lines.is_empty() {
            return;
        }

        if let Some(prev_kind) = *last_segment_kind
            && prev_kind != kind
        {
            target.push(Line::new(String::new()));
        }

        target.extend(lines);
        *last_segment_kind = Some(kind);
    }

    /// Advance the loader animation by one tick. No-op when nothing is animating.
    pub fn on_tick(&mut self) -> std::io::Result<()> {
        if self.waiting_for_response || self.tool_call_statuses.has_running() {
            self.animation_tick = self.animation_tick.wrapping_add(1);
            self.grid_loader.tick = self.animation_tick;
            self.tool_call_statuses.set_tick(self.animation_tick);
            self.render_frame()?;
        }
        Ok(())
    }

    /// Handle an extension notification from ACP.
    pub fn on_ext_notification(&mut self, notification: ExtNotification) -> std::io::Result<()> {
        if notification.method.as_ref() == CONTEXT_USAGE_METHOD {
            if let Some(ratio) =
                serde_json::from_str::<serde_json::Value>(notification.params.get())
                    .ok()
                    .and_then(|v| v.get("usage_ratio")?.as_f64())
            {
                let pct_left = ((1.0 - ratio) * 100.0).round() as u8;
                self.context_usage_pct = Some(pct_left);
                self.render_frame()?;
            }
        }
        Ok(())
    }

    /// Called when the agent prompt errors out.
    pub fn on_prompt_error(&mut self) -> std::io::Result<()> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        self.render_frame()
    }

    pub fn update_render_context(&mut self) {
        self.tui.update_context_from_terminal();
    }

    /// Handle a bracketed paste event: insert all text at once and render once.
    pub fn on_paste(&mut self, text: &str) -> std::io::Result<()> {
        // Close pickers if open — pasted text is treated as literal input
        self.file_picker = None;
        self.command_picker = None;
        self.config_picker = None;
        // Strip newlines from paste (single-line input field)
        for c in text.chars() {
            if !c.is_control() {
                self.input_buffer.push(c);
            }
        }
        self.render_frame()
    }

    /// Handle terminal resize: update context and re-render.
    pub fn on_resize(&mut self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.tui.update_context((cols, rows));
        self.render_frame()
    }

    /// Update render context with provided size (useful for testing)
    #[allow(dead_code)]
    pub fn update_render_context_with(&mut self, size: (u16, u16)) {
        self.tui.update_context(size);
    }

    /// Render the initial frame (just the prompt).
    pub fn initial_render(&mut self) -> std::io::Result<()> {
        self.render_frame()
    }

    /// Get a reference to the screen (useful for testing)
    #[allow(dead_code)]
    pub fn screen(&self) -> &Screen {
        self.tui.screen()
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

const CONTEXT_USAGE_METHOD: &str = "_aether/context_usage";

/// Extract a human-readable model display string from config options.
///
/// Finds the option with `id == "model"`, reads its `Select` kind,
/// and looks up the `current_value` in the options list to return its `name`.
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
