use crate::components::command_picker::CommandEntry;
use crate::components::conversation_window::{
    ConversationBuffer, StreamSegment, StreamSegmentKind, extend_with_vertical_margin,
    render_stream_segment,
};
use crate::components::grid_loader::GridLoader;
use crate::components::screen_view::{ScreenView, ScreenViewAction, ScreenViewRenderProps};
use crate::components::tool_call_statuses::ToolCallStatuses;
use crate::tui::{Line, RenderContext, ScreenLayout};
use agent_client_protocol::{
    self as acp, ExtNotification, SessionConfigKind, SessionConfigOption,
    SessionConfigSelectOptions, SessionUpdate,
};
use crossterm::event::{self, KeyCode, KeyEvent};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use url::Url;

const MAX_EMBED_TEXT_BYTES: usize = 1024 * 1024;
const CONTEXT_USAGE_METHOD: &str = "_aether/context_usage";

pub enum ControllerEffect {
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
}

#[derive(Debug, Clone)]
struct SelectedFileMention {
    mention: String,
    path: PathBuf,
    display_name: String,
}

pub struct ScreenController {
    tool_call_statuses: ToolCallStatuses,
    grid_loader: GridLoader,
    conversation: ConversationBuffer,
    input_buffer: String,
    agent_name: String,
    model_display: Option<String>,
    config_options: Vec<SessionConfigOption>,
    waiting_for_response: bool,
    animation_tick: u16,
    screen_view: ScreenView,
    available_commands: Vec<CommandEntry>,
    pending_open_model_picker: bool,
    selected_mentions: Vec<SelectedFileMention>,
    context_usage_pct: Option<u8>,
}

impl ScreenController {
    pub fn new(agent_name: String, config_options: &[SessionConfigOption]) -> Self {
        Self {
            tool_call_statuses: ToolCallStatuses::new(),
            grid_loader: GridLoader::default(),
            conversation: ConversationBuffer::new(),
            input_buffer: String::new(),
            agent_name,
            model_display: extract_model_display(config_options),
            config_options: config_options.to_vec(),
            waiting_for_response: false,
            animation_tick: 0,
            screen_view: ScreenView::new(),
            available_commands: Vec::new(),
            pending_open_model_picker: false,
            selected_mentions: Vec::new(),
            context_usage_pct: None,
        }
    }

    pub fn on_key_event(&mut self, key_event: KeyEvent) -> Vec<ControllerEffect> {
        if key_event.code == KeyCode::Char('c')
            && key_event.modifiers.contains(event::KeyModifiers::CONTROL)
        {
            return vec![ControllerEffect::Exit];
        }

        let outcome = self
            .screen_view
            .handle_key(key_event, &mut self.input_buffer);
        if outcome.consumed {
            if let Some(action) = outcome.action {
                return self.handle_screen_view_action(action);
            }
            if outcome.needs_render {
                return vec![ControllerEffect::Render];
            }
            return vec![];
        }

        match key_event.code {
            KeyCode::Char('/') if self.input_buffer.is_empty() => {
                self.input_buffer.push('/');
                let mut commands = builtin_commands();
                commands.extend(self.available_commands.clone());
                self.screen_view.open_command_picker(commands);
                vec![ControllerEffect::Render]
            }
            KeyCode::Char('@') => {
                self.input_buffer.push('@');
                self.screen_view.open_file_picker();
                vec![ControllerEffect::Render]
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                vec![ControllerEffect::Render]
            }
            KeyCode::Backspace => {
                if !self.input_buffer.is_empty() {
                    self.input_buffer.pop();
                    vec![ControllerEffect::Render]
                } else {
                    vec![]
                }
            }
            KeyCode::Enter => self.execute_input(),
            _ => vec![],
        }
    }

    pub fn on_session_update(&mut self, update: acp::SessionUpdate) -> Vec<ControllerEffect> {
        let was_loading = self.grid_loader.visible;
        let mut should_render = was_loading;
        self.waiting_for_response = false;
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
                self.config_options = update.config_options.clone();
                self.screen_view.update_config_menu(&update.config_options);
                if self.pending_open_model_picker {
                    self.pending_open_model_picker = false;
                    self.screen_view.open_config_picker_for("model");
                }
                should_render = true;
            }
            _ => {
                self.conversation.close_thought_block();
            }
        }

        if should_render {
            vec![ControllerEffect::Render]
        } else {
            vec![]
        }
    }

    pub fn on_prompt_done(&mut self, render_size: (u16, u16)) -> Vec<ControllerEffect> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        self.conversation.close_thought_block();

        let stream_segments = self.conversation.take_segments();
        let mut remaining_segments = Vec::new();
        let context = RenderContext::new(render_size);

        let mut scrollback_lines: Vec<Line> = Vec::new();
        let mut last_segment_kind: Option<StreamSegmentKind> = None;

        for segment in stream_segments {
            if let StreamSegment::ToolCall(id) = &segment
                && self.tool_call_statuses.is_tool_running(id)
            {
                remaining_segments.push(segment);
                continue;
            }

            let kind = segment.kind();
            let segment_lines = render_stream_segment(&segment, &self.tool_call_statuses, &context);
            extend_with_vertical_margin(
                &mut scrollback_lines,
                &mut last_segment_kind,
                kind,
                segment_lines,
            );

            if let StreamSegment::ToolCall(id) = segment {
                self.tool_call_statuses.remove_tool(&id);
            }
        }

        self.conversation.set_segments(remaining_segments);

        let mut effects = Vec::new();
        if !scrollback_lines.is_empty() {
            effects.push(ControllerEffect::PushScrollback(scrollback_lines));
        }
        effects.push(ControllerEffect::Render);
        effects
    }

    pub fn on_tick(&mut self) -> Vec<ControllerEffect> {
        if self.waiting_for_response || self.tool_call_statuses.has_running() {
            self.animation_tick = self.animation_tick.wrapping_add(1);
            self.grid_loader.tick = self.animation_tick;
            self.tool_call_statuses.set_tick(self.animation_tick);
            vec![ControllerEffect::Render]
        } else {
            vec![]
        }
    }

    pub fn on_ext_notification(&mut self, notification: ExtNotification) -> Vec<ControllerEffect> {
        if notification.method.as_ref() == CONTEXT_USAGE_METHOD
            && let Some(ratio) =
                serde_json::from_str::<serde_json::Value>(notification.params.get())
                    .ok()
                    .and_then(|v| v.get("usage_ratio")?.as_f64())
        {
            let pct_left = ((1.0 - ratio) * 100.0).round() as u8;
            self.context_usage_pct = Some(pct_left);
            return vec![ControllerEffect::Render];
        }
        vec![]
    }

    pub fn on_prompt_error(&mut self) -> Vec<ControllerEffect> {
        self.waiting_for_response = false;
        self.grid_loader.visible = false;
        vec![ControllerEffect::Render]
    }

    pub fn on_paste(&mut self, text: &str) -> Vec<ControllerEffect> {
        self.screen_view.close_all_pickers();
        for c in text.chars() {
            if !c.is_control() {
                self.input_buffer.push(c);
            }
        }
        vec![ControllerEffect::Render]
    }

    pub fn on_resize(&mut self, _cols: u16, _rows: u16) -> Vec<ControllerEffect> {
        vec![ControllerEffect::Render]
    }

    pub fn layout(&self, context: &RenderContext) -> ScreenLayout {
        self.screen_view.layout(self.render_props(), context)
    }

    fn render_props(&self) -> ScreenViewRenderProps<'_> {
        ScreenViewRenderProps {
            loader: &self.grid_loader,
            segments: self.conversation.segments(),
            tool_call_statuses: &self.tool_call_statuses,
            input: &self.input_buffer,
            active_mention_start: self.active_mention_start(),
            agent_name: &self.agent_name,
            model_display: self.model_display.as_deref(),
            context_pct_left: self.context_usage_pct,
        }
    }

    pub fn screen_view(&self) -> &ScreenView {
        &self.screen_view
    }

    pub fn screen_view_mut(&mut self) -> &mut ScreenView {
        &mut self.screen_view
    }

    pub fn available_commands(&self) -> &[CommandEntry] {
        &self.available_commands
    }

    fn handle_screen_view_action(&mut self, action: ScreenViewAction) -> Vec<ControllerEffect> {
        match action {
            ScreenViewAction::FileSelected { path, display_name } => {
                self.apply_file_selection(path, display_name);
                vec![ControllerEffect::Render]
            }
            ScreenViewAction::CommandChosen(cmd) => self.apply_command(cmd),
            ScreenViewAction::ConfigChanged(change) => {
                if change.config_id == "provider" {
                    self.pending_open_model_picker = true;
                }
                vec![
                    ControllerEffect::SetConfigOption {
                        config_id: change.config_id,
                        new_value: change.new_value,
                    },
                    ControllerEffect::Render,
                ]
            }
        }
    }

    fn apply_command(&mut self, cmd: CommandEntry) -> Vec<ControllerEffect> {
        if cmd.builtin && cmd.name == "config" {
            self.input_buffer.clear();
            self.screen_view.close_all_pickers();
            self.pending_open_model_picker = false;
            self.screen_view.open_config_menu(&self.config_options);
            vec![ControllerEffect::Render]
        } else if cmd.has_input {
            self.input_buffer = format!("/{} ", cmd.name);
            vec![ControllerEffect::Render]
        } else {
            self.input_buffer = format!("/{}", cmd.name);
            self.execute_input()
        }
    }

    fn execute_input(&mut self) -> Vec<ControllerEffect> {
        if self.input_buffer.trim().is_empty() {
            return vec![ControllerEffect::Render];
        }

        let user_input = self.input_buffer.trim().to_string();
        self.input_buffer.clear();
        self.screen_view.close_input_pickers();

        let mut effects = vec![ControllerEffect::PushScrollback(vec![Line::new(
            user_input.clone(),
        )])];

        let (content_blocks, warning_lines) = self.build_attachment_blocks(&user_input);
        if !warning_lines.is_empty() {
            effects.push(ControllerEffect::PushScrollback(warning_lines));
        }

        effects.push(ControllerEffect::PromptSubmit {
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

        effects.push(ControllerEffect::Render);
        effects
    }

    fn apply_file_selection(&mut self, path: PathBuf, display_name: String) {
        let mention = format!("@{}", display_name);
        self.selected_mentions.push(SelectedFileMention {
            mention: mention.clone(),
            path,
            display_name,
        });

        if let Some(at_pos) = self.active_mention_start() {
            self.input_buffer.truncate(at_pos);
            self.input_buffer.push_str(&mention);
            self.input_buffer.push(' ');
        }
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

    fn build_attachment_blocks(&mut self, user_input: &str) -> (Vec<acp::ContentBlock>, Vec<Line>) {
        let mentions: HashSet<&str> = user_input.split_whitespace().collect();
        let selected = std::mem::take(&mut self.selected_mentions);
        let mut warning_lines = Vec::new();
        let mut blocks = Vec::new();

        for mention in selected {
            if !mentions.contains(mention.mention.as_str()) {
                continue;
            }

            match self.try_build_attachment_block(&mention.path, &mention.display_name) {
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
