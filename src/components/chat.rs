use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Rect, Size},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, StatefulWidget},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_scrollview::{ScrollView, ScrollViewState};

use super::Component;
use crate::{
    action::{Action, ScrollDirection},
    config::Config,
    theme::Theme,
    types::ChatMessage,
};

pub struct Chat {
    messages: Vec<ChatMessage>,
    scroll_state: ScrollViewState,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    theme: Theme,
    auto_scroll: bool,
    cached_content: Option<Text<'static>>,
    content_dirty: bool,
    last_content_height: u16,
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
            scroll_state: ScrollViewState::default(),
            command_tx: None,
            config: Config::default(),
            theme: Theme::default(),
            auto_scroll: true,
            cached_content: None,
            content_dirty: true,
            last_content_height: 0,
        }
    }

    fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.auto_scroll = true;
        self.content_dirty = true;
    }

    fn auto_scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
    }

    fn clear_messages(&mut self) {
        self.messages.clear();
        self.scroll_state = ScrollViewState::default();
        self.auto_scroll = true;
        self.content_dirty = true;
        self.cached_content = None;
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
        
        // Limit content to prevent performance issues
        const MAX_LINES: usize = 1000;
        let mut line_count = 0;
        let mut truncated = false;

        for line in content.lines() {
            if line_count >= MAX_LINES {
                truncated = true;
                break;
            }
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
            line_count += 1;
        }
        
        if truncated {
            lines.push(Line::from(Span::styled(
                "... [Content truncated for performance]",
                Style::default().fg(self.theme.muted),
            )));
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

    fn create_message_content(&mut self) -> &Text<'static> {
        if self.content_dirty || self.cached_content.is_none() {
            let mut all_lines = Vec::new();
            
            for (i, message) in self.messages.iter().enumerate() {
                let message_lines = self.format_message(message);
                all_lines.extend(message_lines);
                
                // Add spacing between messages (except for last message)
                if i < self.messages.len() - 1 {
                    all_lines.push(Line::from(""));
                }
            }
            
            self.cached_content = Some(Text::from(all_lines));
            self.content_dirty = false;
        }
        
        self.cached_content.as_ref().unwrap()
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
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
        use crossterm::event::KeyModifiers;
        
        match (key.code, key.modifiers) {
            // Ctrl+Up/Down for chat scrolling
            (KeyCode::Up, KeyModifiers::CONTROL) => Ok(Some(Action::ScrollChat(ScrollDirection::Up))),
            (KeyCode::Down, KeyModifiers::CONTROL) => Ok(Some(Action::ScrollChat(ScrollDirection::Down))),
            // Page keys always work for chat scrolling
            (KeyCode::PageUp, _) => Ok(Some(Action::ScrollChat(ScrollDirection::PageUp))),
            (KeyCode::PageDown, _) => Ok(Some(Action::ScrollChat(ScrollDirection::PageDown))),
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
                // Disable auto-scroll when user manually scrolls
                self.auto_scroll = false;
                
                match direction {
                    ScrollDirection::Up => {
                        self.scroll_state.scroll_up();
                    }
                    ScrollDirection::Down => {
                        self.scroll_state.scroll_down();
                    }
                    ScrollDirection::PageUp => {
                        for _ in 0..5 {
                            self.scroll_state.scroll_up();
                        }
                    }
                    ScrollDirection::PageDown => {
                        for _ in 0..5 {
                            self.scroll_state.scroll_down();
                        }
                    }
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
                    // Enable auto-scroll for streaming content
                    self.auto_scroll = true;
                    self.content_dirty = true;
                }
            }
            Action::StreamComplete => {
                // Convert streaming message to final message
                if let Some(ChatMessage::AssistantStreaming { content, timestamp }) =
                    self.messages.last().cloned()
                {
                    if let Some(last_msg) = self.messages.last_mut() {
                        *last_msg = ChatMessage::Assistant { content, timestamp };
                        self.content_dirty = true;
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
        // Calculate the inner area (accounting for borders)
        let inner_area = area.inner(ratatui::layout::Margin { horizontal: 1, vertical: 1 });
        let content_width = inner_area.width;
        
        // Store values we need before borrowing mutably
        let last_height = self.last_content_height;
        let auto_scroll = self.auto_scroll;
        
        // Get content and calculate height
        let content = self.create_message_content().clone();
        let content_height = content.lines.len() as u16;
        
        // Only recreate scroll view if content height changed
        let mut scroll_view = if content_height != last_height {
            self.last_content_height = content_height;
            ScrollView::new(Size::new(content_width, content_height))
        } else {
            // Reuse existing dimensions
            ScrollView::new(Size::new(content_width, last_height))
        };
        
        // Handle auto-scroll logic
        if auto_scroll {
            self.scroll_state.scroll_to_bottom();
            self.auto_scroll = false;
        }
        
        // Create the paragraph without borders (borders are handled by the outer block)
        let paragraph = Paragraph::new(content)
            .wrap(ratatui::widgets::Wrap { trim: false });
        
        // Create the outer block with borders and title
        let block = Block::default().borders(Borders::ALL).title("Chat");
        frame.render_widget(block, area);
        
        // Render the scrollable content in the inner area
        let content_area = Rect::new(0, 0, content_width, content_height);
        scroll_view.render_widget(paragraph, content_area);
        
        scroll_view.render(inner_area, frame.buffer_mut(), &mut self.scroll_state);
        Ok(())
    }
}
