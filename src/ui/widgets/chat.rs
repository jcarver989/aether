use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget, Wrap},
};

use crate::llm::provider::ChatMessage;

pub struct ChatWidget<'a> {
    messages: &'a [ChatMessage],
    block: Option<Block<'a>>,
}

impl<'a> ChatWidget<'a> {
    pub fn new(messages: &'a [ChatMessage]) -> Self {
        Self {
            messages,
            block: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    fn format_message(&self, message: &ChatMessage) -> Vec<Line<'a>> {
        match message {
            ChatMessage::System { content } => {
                vec![
                    Line::from(vec![
                        Span::styled("System", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::raw(": "),
                        Span::styled(content.clone(), Style::default().fg(Color::Gray)),
                    ])
                ]
            }
            ChatMessage::User { content } => {
                vec![
                    Line::from(vec![
                        Span::styled("You", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(": "),
                        Span::raw(content.clone()),
                    ])
                ]
            }
            ChatMessage::Assistant { content } => {
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Assistant", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                        Span::raw(": "),
                    ])
                ];
                
                // Simple markdown-style formatting
                let formatted_lines = self.format_assistant_content(content);
                lines.extend(formatted_lines);
                lines
            }
            ChatMessage::Tool { tool_call_id, content } => {
                vec![
                    Line::from(vec![
                        Span::styled("Tool", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" ("),
                        Span::styled(tool_call_id.clone(), Style::default().fg(Color::Gray)),
                        Span::raw("): "),
                        Span::styled(content.clone(), Style::default().fg(Color::Cyan)),
                    ])
                ]
            }
        }
    }

    fn format_assistant_content(&self, content: &str) -> Vec<Line<'a>> {
        let mut lines = Vec::new();
        let mut in_code_block = false;
        let mut code_language = String::new();

        for line in content.lines() {
            if line.starts_with("```") {
                if in_code_block {
                    // End of code block
                    in_code_block = false;
                    lines.push(Line::from(Span::styled("```", Style::default().fg(Color::DarkGray))));
                } else {
                    // Start of code block
                    in_code_block = true;
                    code_language = line.trim_start_matches("```").to_string();
                    lines.push(Line::from(vec![
                        Span::styled("```", Style::default().fg(Color::DarkGray)),
                        Span::styled(code_language.clone(), Style::default().fg(Color::Yellow)),
                    ]));
                }
            } else if in_code_block {
                // Code line
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(Color::Cyan).bg(Color::DarkGray),
                )));
            } else {
                // Regular text with basic markdown formatting
                let formatted_line = self.format_markdown_line(line);
                lines.push(formatted_line);
            }
        }

        lines
    }

    fn format_markdown_line(&self, line: &str) -> Line<'a> {
        let mut spans = Vec::new();
        let mut chars = line.chars().peekable();
        let mut current_text = String::new();
        let mut in_bold = false;
        let mut in_italic = false;
        let mut in_code = false;

        while let Some(ch) = chars.next() {
            match ch {
                '*' if chars.peek() == Some(&'*') && !in_code => {
                    // Bold
                    if !current_text.is_empty() {
                        let style = if in_italic { Style::default().add_modifier(Modifier::ITALIC) } else { Style::default() };
                        spans.push(Span::styled(current_text.clone(), style));
                        current_text.clear();
                    }
                    chars.next(); // consume second *
                    in_bold = !in_bold;
                }
                '*' if !in_code => {
                    // Italic
                    if !current_text.is_empty() {
                        let style = if in_bold { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };
                        spans.push(Span::styled(current_text.clone(), style));
                        current_text.clear();
                    }
                    in_italic = !in_italic;
                }
                '`' if !in_bold && !in_italic => {
                    // Inline code
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

        // Add remaining text
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

impl<'a> Widget for ChatWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut all_lines = Vec::new();

        for (i, message) in self.messages.iter().enumerate() {
            if i > 0 {
                // Add separator between messages
                all_lines.push(Line::from(""));
            }
            all_lines.extend(self.format_message(message));
        }

        let text = Text::from(all_lines);
        let mut paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .scroll((0, 0));

        if let Some(block) = self.block {
            paragraph = paragraph.block(block);
        }

        paragraph.render(area, buf);
    }
}