use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{action::{Action, ScrollDirection}, config::Config, types::ChatMessage};

pub struct Chat {
    messages: Vec<ChatMessage>,
    scroll_offset: u16,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
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
            scroll_offset: 0,
            command_tx: None,
            config: Config::default(),
        }
    }

    fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    fn clear_messages(&mut self) {
        self.messages.clear();
        self.scroll_offset = 0;
    }

    fn format_message(&self, message: &ChatMessage) -> Vec<Line<'static>> {
        match message {
            ChatMessage::System { content, timestamp } => {
                vec![
                    Line::from(vec![
                        Span::styled("System", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::raw(" ("),
                        Span::styled(timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
                        Span::raw("): "),
                        Span::styled(content.clone(), Style::default().fg(Color::Gray)),
                    ])
                ]
            }
            ChatMessage::User { content, timestamp } => {
                vec![
                    Line::from(vec![
                        Span::styled("You", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(" ("),
                        Span::styled(timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
                        Span::raw("): "),
                        Span::raw(content.clone()),
                    ])
                ]
            }
            ChatMessage::Assistant { content, timestamp } => {
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Assistant", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                        Span::raw(" ("),
                        Span::styled(timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
                        Span::raw("): "),
                    ])
                ];
                
                let formatted_lines = self.format_assistant_content(content);
                lines.extend(formatted_lines);
                lines
            }
            ChatMessage::AssistantStreaming { content, timestamp } => {
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Assistant", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                        Span::raw(" ("),
                        Span::styled(timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
                        Span::raw("): "),
                    ])
                ];
                
                let mut formatted_lines = self.format_assistant_content(content);
                // Add cursor indicator for streaming
                if let Some(last_line) = formatted_lines.last_mut() {
                    let mut spans = last_line.spans.clone();
                    spans.push(Span::styled(" ▋", Style::default().fg(Color::Gray)));
                    *last_line = Line::from(spans);
                } else {
                    formatted_lines.push(Line::from(Span::styled(" ▋", Style::default().fg(Color::Gray))));
                }
                lines.extend(formatted_lines);
                lines
            }
            ChatMessage::Tool { tool_call_id, content, timestamp } => {
                vec![
                    Line::from(vec![
                        Span::styled("Tool", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" ("),
                        Span::styled(tool_call_id.clone(), Style::default().fg(Color::Gray)),
                        Span::raw(") "),
                        Span::styled(timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
                        Span::raw(": "),
                        Span::styled(content.clone(), Style::default().fg(Color::Cyan)),
                    ])
                ]
            }
            ChatMessage::ToolCall { name, params, timestamp } => {
                vec![
                    Line::from(vec![
                        Span::styled("Tool Call", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw(" ("),
                        Span::styled(timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
                        Span::raw("): "),
                        Span::raw(format!("{}({})", name, params)),
                    ])
                ]
            }
            ChatMessage::ToolResult { content, timestamp } => {
                vec![
                    Line::from(vec![
                        Span::styled("Result", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::raw(" ("),
                        Span::styled(timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
                        Span::raw("): "),
                        Span::raw(content.clone()),
                    ])
                ]
            }
            ChatMessage::Error { message, timestamp } => {
                vec![
                    Line::from(vec![
                        Span::styled("Error", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::raw(" ("),
                        Span::styled(timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
                        Span::raw("): "),
                        Span::raw(message.clone()),
                    ])
                ]
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
                    lines.push(Line::from(Span::styled("```", Style::default().fg(Color::DarkGray))));
                } else {
                    in_code_block = true;
                    code_language = line.trim_start_matches("```").to_string();
                    lines.push(Line::from(vec![
                        Span::styled("```", Style::default().fg(Color::DarkGray)),
                        Span::styled(code_language.clone(), Style::default().fg(Color::Yellow)),
                    ]));
                }
            } else if in_code_block {
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(Color::Cyan).bg(Color::DarkGray),
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
                        let style = if in_italic { Style::default().add_modifier(Modifier::ITALIC) } else { Style::default() };
                        spans.push(Span::styled(current_text.clone(), style));
                        current_text.clear();
                    }
                    chars.next();
                    in_bold = !in_bold;
                }
                '*' if !in_code => {
                    if !current_text.is_empty() {
                        let style = if in_bold { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };
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
            if in_bold { style = style.add_modifier(Modifier::BOLD); }
            if in_italic { style = style.add_modifier(Modifier::ITALIC); }
            if in_code { style = style.fg(Color::Cyan).bg(Color::DarkGray); }
            
            spans.push(Span::styled(current_text, style));
        }

        if spans.is_empty() {
            Line::from("")
        } else {
            Line::from(spans)
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

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            Action::Render => {}
            Action::AddChatMessage(message) => {
                self.add_message(message);
                // Auto-scroll to bottom when new message is added
                self.scroll_offset = 0;
            }
            Action::ClearChat => {
                self.clear_messages();
            }
            Action::ScrollChat(direction) => {
                match direction {
                    ScrollDirection::Up => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    }
                    ScrollDirection::Down => {
                        self.scroll_offset = self.scroll_offset.saturating_add(1);
                    }
                    ScrollDirection::PageUp => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(10);
                    }
                    ScrollDirection::PageDown => {
                        self.scroll_offset = self.scroll_offset.saturating_add(10);
                    }
                }
            }
            Action::StartStreaming => {
                // Add initial streaming message
                self.add_message(ChatMessage::AssistantStreaming { 
                    content: String::new(),
                    timestamp: chrono::Utc::now(),
                });
                self.scroll_offset = 0; // Auto-scroll to bottom
            }
            Action::StreamContent(content) => {
                // Update the last streaming message
                if let Some(ChatMessage::AssistantStreaming { content: current_content, timestamp: _ }) = self.messages.last_mut() {
                    current_content.push_str(&content);
                }
            }
            Action::StreamComplete => {
                // Convert streaming message to final message
                if let Some(ChatMessage::AssistantStreaming { content, timestamp }) = self.messages.last().cloned() {
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
                self.scroll_offset = 0;
            }
            Action::StreamToolCall { id: _, name, arguments } => {
                self.add_message(ChatMessage::ToolCall { 
                    name, 
                    params: arguments,
                    timestamp: chrono::Utc::now(),
                });
                self.scroll_offset = 0;
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let mut all_lines = Vec::new();

        for (i, message) in self.messages.iter().enumerate() {
            if i > 0 {
                all_lines.push(Line::from(""));
            }
            all_lines.extend(self.format_message(message));
        }

        let text = Text::from(all_lines);
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Chat"))
            .wrap(Wrap { trim: true })
            .scroll((self.scroll_offset, 0));

        frame.render_widget(paragraph, area);
        Ok(())
    }
}