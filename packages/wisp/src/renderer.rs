use crate::components::file_picker::{FilePicker, FilePickerComponent};
use crate::components::grid_loader::GridLoader;
use crate::components::input_prompt::InputPrompt;
use crate::components::status_line::StatusLine;
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::error::WispError;
use crate::tui::soft_wrap::soft_wrap_lines_with_map;
use crate::tui::{Component, FrameRenderer, Line, RenderContext, Screen};
use acp_utils::client::AcpPromptHandle;
use agent_client_protocol::{
    self as acp, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions, SessionUpdate,
};
use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
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

struct FrameSnapshot {
    visual_lines: Vec<Line>,
    cursor_row: usize,
    cursor_col: u16,
}

pub struct Renderer<T: Write> {
    tui: FrameRenderer<T>,
    tool_call_statuses: ToolCallStatuses,
    grid_loader: GridLoader,
    current_message_buffer: String,
    input_buffer: String,
    agent_name: String,
    model_display: Option<String>,
    waiting_for_response: bool,
    animation_tick: u16,
    pub file_picker: Option<FilePicker>,
    selected_mentions: Vec<SelectedFileMention>,
}

impl<T: Write> Renderer<T> {
    pub fn new(writer: T, agent_name: String, config_options: &[SessionConfigOption]) -> Self {
        Self {
            tui: FrameRenderer::new(writer),
            tool_call_statuses: ToolCallStatuses::new(),
            grid_loader: GridLoader::new(),
            current_message_buffer: String::new(),
            input_buffer: String::new(),
            agent_name,
            model_display: extract_model_display(config_options),
            waiting_for_response: false,
            animation_tick: 0,
            file_picker: None,
            selected_mentions: Vec::new(),
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
        logical_lines.extend(self.tool_call_statuses.render(context));
        logical_lines.extend(self.grid_loader.render(context));

        if !self.current_message_buffer.is_empty() {
            let streaming_message = StreamingMessage {
                text: &self.current_message_buffer,
            };
            logical_lines.extend(streaming_message.render(context));
        }

        let input_prompt = InputPrompt {
            input: &self.input_buffer,
            cursor_index: self.input_cursor_index(),
        };
        let input_layout = input_prompt.layout(context);
        let input_cursor_logical_row = logical_lines.len() + input_layout.cursor_row;
        let input_cursor_col = input_layout.cursor_col;
        logical_lines.extend(input_layout.lines);

        if let Some(ref picker) = self.file_picker {
            let picker_component = FilePickerComponent { picker };
            logical_lines.extend(picker_component.render(context));
        }

        let status_line = StatusLine {
            agent_name: &self.agent_name,
            model_display: self.model_display.as_deref(),
        };
        logical_lines.extend(status_line.render(context));

        let (visual_lines, logical_to_visual) =
            soft_wrap_lines_with_map(&logical_lines, context.size.0);

        let mut cursor_row = logical_to_visual
            .get(input_cursor_logical_row)
            .copied()
            .unwrap_or_else(|| visual_lines.len().saturating_sub(1));
        let width = context.size.0 as usize;
        let mut cursor_col = input_cursor_col as usize;
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

    fn input_cursor_index(&self) -> usize {
        if let Some(ref picker) = self.file_picker {
            let at_pos = self
                .active_mention_start()
                .unwrap_or_else(|| self.input_buffer.len());
            at_pos + 1 + picker.query.len()
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
                KeyCode::Up | KeyCode::Char('p')
                    if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    picker.move_selection_up();
                    self.render_frame()?;
                    return Ok(LoopAction::Continue);
                }
                KeyCode::Down | KeyCode::Char('n')
                    if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    picker.move_selection_down();
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
                    picker.update_query(query);
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
                            picker.update_query(query);
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

        match key_event.code {
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
                if self.input_buffer.trim().is_empty() {
                    // Just re-render the prompt
                    self.render_frame()?;
                } else {
                    let user_input = self.input_buffer.trim().to_string();
                    self.input_buffer.clear();
                    self.file_picker = None;

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
            }
            _ => {}
        }
        Ok(LoopAction::Continue)
    }

    fn confirm_file_selection(&mut self) -> Result<(), WispError> {
        let picker = self
            .file_picker
            .take()
            .ok_or(WispError::Other("File picker not active".into()))?;

        if let Some(selected) = picker.files.get(picker.selected_index) {
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
                    self.current_message_buffer.push_str(&text_content.text);
                    should_render = true;
                }
            }

            SessionUpdate::ToolCall(tool_call) => {
                self.tool_call_statuses.on_tool_call(&tool_call);
                should_render = true;
            }

            SessionUpdate::ToolCallUpdate(update) => {
                self.tool_call_statuses.on_tool_call_update(&update);
                should_render = true;
            }

            SessionUpdate::ConfigOptionUpdate(update) => {
                self.model_display = extract_model_display(&update.config_options);
                should_render = true;
            }

            _ => {}
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

        let text = std::mem::take(&mut self.current_message_buffer);

        let mut scrollback_lines: Vec<Line> =
            self.tool_call_statuses.drain_completed(self.tui.context());

        if !text.is_empty() {
            for text_line in text.lines() {
                scrollback_lines.push(Line::new(text_line.to_string()));
            }
        }

        if !scrollback_lines.is_empty() {
            self.tui.push_to_scrollback(&scrollback_lines)?;
        }

        self.render_frame()?;
        Ok(())
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

    /// Called when the agent prompt errors out.
    pub fn on_prompt_error(&mut self) -> std::io::Result<()> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        self.render_frame()
    }

    pub fn update_render_context(&mut self) {
        self.tui.update_context_from_terminal();
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

/// Extract a human-readable model display string from config options.
///
/// Finds the first option with `category == Model`, reads its `Select` kind,
/// and looks up the `current_value` in the options list to return its `name`.
fn extract_model_display(config_options: &[SessionConfigOption]) -> Option<String> {
    let option = config_options.iter().find(|o| {
        o.category
            .as_ref()
            .is_some_and(|c| *c == SessionConfigOptionCategory::Model)
    })?;

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
