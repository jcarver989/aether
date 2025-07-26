use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::llm::provider::ToolCall;

#[derive(Debug, Clone, Copy)]
pub enum ToolCallState {
    Pending,
    Running,
    Completed,
    Failed,
}

pub struct ToolCallWidget<'a> {
    tool_call: &'a ToolCall,
    state: ToolCallState,
    result: Option<&'a str>,
    expanded: bool,
    block: Option<Block<'a>>,
}

impl<'a> ToolCallWidget<'a> {
    pub fn new(tool_call: &'a ToolCall) -> Self {
        Self {
            tool_call,
            state: ToolCallState::Pending,
            result: None,
            expanded: false,
            block: None,
        }
    }

    pub fn with_state(mut self, state: ToolCallState) -> Self {
        self.state = state;
        self
    }

    pub fn with_result(mut self, result: &'a str) -> Self {
        self.result = Some(result);
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    fn get_state_symbol_and_color(&self) -> (char, Color) {
        match self.state {
            ToolCallState::Pending => ('○', Color::Yellow),
            ToolCallState::Running => ('◐', Color::Blue),
            ToolCallState::Completed => ('●', Color::Green),
            ToolCallState::Failed => ('●', Color::Red),
        }
    }

    fn format_arguments(&self) -> Vec<Line<'a>> {
        if !self.expanded {
            return vec![];
        }

        let mut lines = vec![
            Line::from(Span::styled(
                "  Parameters:",
                Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD),
            ))
        ];

        if let Some(obj) = self.tool_call.arguments.as_object() {
            for (key, value) in obj {
                let formatted_value = match value {
                    serde_json::Value::String(s) => {
                        if s.len() > 50 {
                            format!("\"{}...\"", &s[..47])
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => "null".to_string(),
                    _ => serde_json::to_string(value).unwrap_or_else(|_| "...".to_string()),
                };

                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(key.clone(), Style::default().fg(Color::Cyan)),
                    Span::raw(": "),
                    Span::styled(formatted_value, Style::default().fg(Color::White)),
                ]));
            }
        } else {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    serde_json::to_string_pretty(&self.tool_call.arguments)
                        .unwrap_or_else(|_| "Invalid JSON".to_string()),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }

        lines
    }

    fn format_result(&self) -> Vec<Line<'a>> {
        if let Some(result) = self.result {
            let mut lines = vec![
                Line::from(Span::styled(
                    "  Result:",
                    Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD),
                ))
            ];

            // Show first few lines of result
            let result_lines: Vec<&str> = result.lines().take(5).collect();
            for line in result_lines {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(line.to_string(), Style::default().fg(Color::Green)),
                ]));
            }

            if result.lines().count() > 5 {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled("... (truncated)", Style::default().fg(Color::Gray)),
                ]));
            }

            lines
        } else {
            vec![]
        }
    }
}

impl<'a> Widget for ToolCallWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (symbol, color) = self.get_state_symbol_and_color();
        let expand_symbol = if self.expanded { "▼" } else { "▶" };

        let mut content = vec![
            Line::from(vec![
                Span::styled(symbol.to_string(), Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(expand_symbol, Style::default().fg(Color::Gray)),
                Span::raw(" "),
                Span::styled("Tool Call:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(self.tool_call.name.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ])
        ];

        if !self.tool_call.id.is_empty() {
            content.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("ID: ", Style::default().fg(Color::Gray)),
                Span::styled(self.tool_call.id.clone(), Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Add arguments if expanded
        content.extend(self.format_arguments());

        // Add result if available
        content.extend(self.format_result());

        let text = Text::from(content);
        let mut paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: true });

        if let Some(block) = self.block {
            paragraph = paragraph.block(block);
        }

        paragraph.render(area, buf);
    }
}