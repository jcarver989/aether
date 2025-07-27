use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    action::{Action, ScrollDirection},
    config::Config,
    theme::Theme,
    types::ChatMessage,
};

pub struct Chat {
    messages: Vec<ChatMessage>,
    list_state: ListState,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    theme: Theme,
    total_lines: usize,
    message_line_counts: Vec<usize>,
}

impl Default for Chat {
    fn default() -> Self {
        Self::new()
    }
}

impl Chat {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            list_state: ListState::default(),
            command_tx: None,
            config: Config::default(),
            theme: Theme::default(),
            total_lines: 0,
            message_line_counts: Vec::new(),
        }
    }

    fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.auto_scroll_to_bottom();
    }

    fn auto_scroll_to_bottom(&mut self) {
        if self.total_lines > 0 {
            self.list_state.select(Some(self.total_lines - 1));
        }
    }

    fn clear_messages(&mut self) {
        self.messages.clear();
        self.message_line_counts.clear();
        self.total_lines = 0;
        self.list_state.select(None);
    }

    fn format_message(&self, message: &ChatMessage) -> Vec<Line<'static>> {
        match message {
            ChatMessage::System { content, timestamp } => {
                vec![Line::from(vec![
                    Span::styled(
                        "System",
                        Style::default()
                            .fg(self.theme.system_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                    Span::styled(content.clone(), Style::default().fg(self.theme.subtle)),
                ])]
            }
            ChatMessage::User { content, timestamp } => {
                vec![Line::from(vec![
                    Span::styled(
                        "You",
                        Style::default()
                            .fg(self.theme.user_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                    Span::styled(content.clone(), Style::default().fg(self.theme.foreground)),
                ])]
            }
            ChatMessage::Assistant { content, timestamp } => {
                let mut lines = vec![Line::from(vec![
                    Span::styled(
                        "Assistant",
                        Style::default()
                            .fg(self.theme.assistant_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                ])];

                let formatted_lines = self.format_assistant_content(content);
                lines.extend(formatted_lines);
                lines
            }
            ChatMessage::AssistantStreaming { content, timestamp } => {
                let mut lines = vec![Line::from(vec![
                    Span::styled(
                        "Assistant",
                        Style::default()
                            .fg(self.theme.assistant_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                ])];

                let mut formatted_lines = self.format_assistant_content(content);
                // Add cursor indicator for streaming
                if let Some(last_line) = formatted_lines.last_mut() {
                    let mut spans = last_line.spans.clone();
                    spans.push(Span::styled(" ▋", Style::default().fg(self.theme.cursor_color)));
                    *last_line = Line::from(spans);
                } else {
                    formatted_lines.push(Line::from(Span::styled(
                        " ▋",
                        Style::default().fg(self.theme.cursor_color),
                    )));
                }
                lines.extend(formatted_lines);
                lines
            }
            ChatMessage::Tool {
                tool_call_id,
                content,
                timestamp,
            } => {
                vec![Line::from(vec![
                    Span::styled(
                        "Tool",
                        Style::default()
                            .fg(self.theme.tool_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(tool_call_id.clone(), Style::default().fg(self.theme.subtle)),
                    Span::raw(") "),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw(": "),
                    Span::styled(content.clone(), Style::default().fg(self.theme.success)),
                ])]
            }
            ChatMessage::ToolCall {
                id,
                name,
                params,
                timestamp,
            } => {
                vec![Line::from(vec![
                    Span::styled(
                        "Tool Call",
                        Style::default()
                            .fg(self.theme.tool_call_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(id.clone(), Style::default().fg(self.theme.subtle)),
                    Span::raw(") "),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw(": "),
                    Span::styled(format!("{}({})", name, params), Style::default().fg(self.theme.foreground)),
                ])]
            }
            ChatMessage::ToolResult {
                tool_call_id,
                content,
                timestamp,
            } => {
                vec![Line::from(vec![
                    Span::styled(
                        "Result",
                        Style::default()
                            .fg(self.theme.tool_result_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(tool_call_id.clone(), Style::default().fg(self.theme.subtle)),
                    Span::raw(") "),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw(": "),
                    Span::styled(content.clone(), Style::default().fg(self.theme.foreground)),
                ])]
            }
            ChatMessage::Error { message, timestamp } => {
                vec![Line::from(vec![
                    Span::styled(
                        "Error",
                        Style::default()
                            .fg(self.theme.error)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ("),
                    Span::styled(
                        timestamp.format("%H:%M:%S").to_string(),
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::raw("): "),
                    Span::styled(message.clone(), Style::default().fg(self.theme.error)),
                ])]
            }
        }
    }

    fn format_assistant_content(&self, content: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let mut in_code_block = false;
        let mut code_language = String::new();

        for line in content.lines() {
            if line.starts_with("```") {
                if in_code_block {
                    in_code_block = false;
                    code_language.clear();
                    lines.push(Line::from(Span::styled(
                        "```",
                        Style::default().fg(self.theme.muted),
                    )));
                } else {
                    in_code_block = true;
                    code_language = line.trim_start_matches("```").to_string();
                    lines.push(Line::from(vec![
                        Span::styled("```", Style::default().fg(self.theme.muted)),
                        Span::styled(code_language.clone(), Style::default().fg(self.theme.warning)),
                    ]));
                }
            } else if in_code_block {
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(self.theme.code_fg).bg(self.theme.code_bg),
                )));
            } else {
                let formatted_line = self.format_markdown_line(line);
                lines.push(formatted_line);
            }
        }

        lines
    }

    fn format_markdown_line(&self, line: &str) -> Line<'static> {
        let mut spans = Vec::new();
        let mut chars = line.chars().peekable();
        let mut current_text = String::new();
        let mut in_bold = false;
        let mut in_italic = false;
        let mut in_code = false;

        while let Some(ch) = chars.next() {
            match ch {
                '*' if chars.peek() == Some(&'*') && !in_code => {
                    if !current_text.is_empty() {
                        let style = if in_italic {
                            Style::default().add_modifier(Modifier::ITALIC)
                        } else {
                            Style::default()
                        };
                        spans.push(Span::styled(current_text.clone(), style));
                        current_text.clear();
                    }
                    chars.next();
                    in_bold = !in_bold;
                }
                '*' if !in_code => {
                    if !current_text.is_empty() {
                        let style = if in_bold {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        spans.push(Span::styled(current_text.clone(), style));
                        current_text.clear();
                    }
                    in_italic = !in_italic;
                }
                '`' if !in_bold && !in_italic => {
                    if !current_text.is_empty() {
                        spans.push(Span::raw(current_text.clone()));
                        current_text.clear();
                    }
                    in_code = !in_code;
                }
                _ => {
                    current_text.push(ch);
                }
            }
        }

        if !current_text.is_empty() {
            let mut style = Style::default();
            if in_bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if in_italic {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if in_code {
                style = style.fg(self.theme.code_fg).bg(self.theme.code_bg);
            }

            spans.push(Span::styled(current_text, style));
        }

        if spans.is_empty() {
            Line::from("")
        } else {
            Line::from(spans)
        }
    }

    fn wrap_lines(&self, lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
        let mut wrapped_lines = Vec::new();
        
        for line in lines {
            let content: String = line.spans.iter()
                .map(|span| span.content.as_ref())
                .collect();
            
            if content.chars().count() <= width {
                wrapped_lines.push(line);
            } else {
                // Word wrap the line
                let words: Vec<&str> = content.split_whitespace().collect();
                let mut current_line = String::new();
                
                for word in words {
                    let test_line = if current_line.is_empty() {
                        word.to_string()
                    } else {
                        format!("{} {}", current_line, word)
                    };
                    
                    if test_line.chars().count() <= width {
                        current_line = test_line;
                    } else {
                        if !current_line.is_empty() {
                            wrapped_lines.push(Line::from(current_line));
                            current_line = word.to_string();
                        } else {
                            // Word is longer than width, add it anyway
                            wrapped_lines.push(Line::from(word.to_string()));
                        }
                    }
                }
                
                if !current_line.is_empty() {
                    wrapped_lines.push(Line::from(current_line));
                }
            }
        }
        
        wrapped_lines
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    fn update_line_counts(&mut self, text_width: usize) {
        self.message_line_counts.clear();
        self.total_lines = 0;
        
        for (i, message) in self.messages.iter().enumerate() {
            let lines = self.format_message(message);
            let mut wrapped_lines = self.wrap_lines(lines, text_width);
            
            // Add empty line for vertical spacing (except for last item)
            if i < self.messages.len() - 1 {
                wrapped_lines.push(Line::from(""));
            }
            
            let line_count = wrapped_lines.len();
            self.message_line_counts.push(line_count);
            self.total_lines += line_count;
        }
    }
}

impl Component for Chat {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Up => Ok(Some(Action::ScrollChat(ScrollDirection::Up))),
            KeyCode::Down => Ok(Some(Action::ScrollChat(ScrollDirection::Down))),
            KeyCode::PageUp => Ok(Some(Action::ScrollChat(ScrollDirection::PageUp))),
            KeyCode::PageDown => Ok(Some(Action::ScrollChat(ScrollDirection::PageDown))),
            _ => Ok(None),
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<Option<Action>> {
        match mouse.kind {
            MouseEventKind::ScrollUp => Ok(Some(Action::ScrollChat(ScrollDirection::Up))),
            MouseEventKind::ScrollDown => Ok(Some(Action::ScrollChat(ScrollDirection::Down))),
            _ => Ok(None),
        }
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            Action::Render => {}
            Action::AddChatMessage(message) => {
                self.add_message(message);
            }
            Action::ClearChat => {
                self.clear_messages();
            }
            Action::ScrollChat(direction) => {
                let current_index = self.list_state.selected().unwrap_or(0);
                let new_index = match direction {
                    ScrollDirection::Up => {
                        current_index.saturating_sub(1)
                    }
                    ScrollDirection::Down => {
                        if current_index + 1 < self.total_lines {
                            current_index + 1
                        } else {
                            current_index
                        }
                    }
                    ScrollDirection::PageUp => {
                        current_index.saturating_sub(5)
                    }
                    ScrollDirection::PageDown => {
                        let new_idx = current_index + 5;
                        if new_idx < self.total_lines {
                            new_idx
                        } else {
                            self.total_lines.saturating_sub(1)
                        }
                    }
                };
                
                if self.total_lines > 0 {
                    self.list_state.select(Some(new_index));
                }
            }
            Action::StartStreaming => {
                // Add initial streaming message
                self.add_message(ChatMessage::AssistantStreaming {
                    content: String::new(),
                    timestamp: chrono::Utc::now(),
                });
            }
            Action::StreamContent(content) => {
                // Update the last streaming message
                if let Some(ChatMessage::AssistantStreaming {
                    content: current_content,
                    timestamp: _,
                }) = self.messages.last_mut()
                {
                    current_content.push_str(&content);
                }
            }
            Action::StreamComplete => {
                // Convert streaming message to final message
                if let Some(ChatMessage::AssistantStreaming { content, timestamp }) =
                    self.messages.last().cloned()
                {
                    if let Some(last_msg) = self.messages.last_mut() {
                        *last_msg = ChatMessage::Assistant { content, timestamp };
                    }
                }
            }
            Action::Error(error) => {
                self.add_message(ChatMessage::Error {
                    message: error,
                    timestamp: chrono::Utc::now(),
                });
            }
            Action::StreamToolCall {
                id,
                name,
                arguments,
            } => {
                // Check if we already have a streaming tool call with this ID
                let mut found_existing = false;
                for message in self.messages.iter_mut().rev() {
                    if let ChatMessage::ToolCall {
                        id: existing_id,
                        name: _existing_name,
                        params,
                        ..
                    } = message
                    {
                        if existing_id == &id {
                            // Update the existing tool call with new arguments
                            *params = arguments.clone();
                            found_existing = true;
                            break;
                        }
                    } else {
                        // Stop looking once we hit a non-tool-call message
                        break;
                    }
                }
                
                // If no existing tool call found, create a new one
                if !found_existing {
                    self.add_message(ChatMessage::ToolCall {
                        id,
                        name,
                        params: arguments,
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Calculate available width for text (account for borders)
        let text_width = area.width.saturating_sub(2) as usize;
        
        // Rebuild line counts if messages have changed
        self.update_line_counts(text_width);
        
        let items: Vec<ListItem> = self
            .messages
            .iter()
            .enumerate()
            .map(|(i, message)| {
                let lines = self.format_message(message);
                let mut wrapped_lines = self.wrap_lines(lines, text_width);
                
                // Add empty line for vertical spacing (except for last item)
                if i < self.messages.len() - 1 {
                    wrapped_lines.push(Line::from(""));
                }
                
                ListItem::new(Text::from(wrapped_lines))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Chat"))
            .highlight_style(
                Style::default()
                    .bg(self.theme.selection_bg)
                    .fg(self.theme.selection_fg)
                    .add_modifier(Modifier::BOLD)
            );

        frame.render_stateful_widget(list, area, &mut self.list_state);
        Ok(())
    }
}
