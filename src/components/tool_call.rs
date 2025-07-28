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

use std::sync::Arc;
use super::Component;
use crate::{
    action::Action,
    config::Config,
    types::{ToolCall, ToolCallState},
};

pub struct ToolCallComponent {
    tool_call: ToolCall,
    state: ToolCallState,
    result: Option<String>,
    expanded: bool,
    command_tx: Option<UnboundedSender<Action>>,
    config: Arc<Config>,
}

impl ToolCallComponent {
    fn set_state(&mut self, state: ToolCallState) {
        self.state = state;
    }

    fn set_result(&mut self, result: String) {
        self.result = Some(result);
    }

    fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    fn get_state_symbol_and_color(&self) -> (char, Color) {
        match self.state {
            ToolCallState::Pending => ('○', Color::Yellow),
            ToolCallState::Running => ('◐', Color::Blue),
            ToolCallState::Completed => ('●', Color::Green),
            ToolCallState::Failed => ('●', Color::Red),
        }
    }

    fn format_arguments(&self) -> Vec<Line<'static>> {
        if !self.expanded {
            return vec![];
        }

        let mut lines = vec![Line::from(Span::styled(
            "  Parameters:",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        ))];

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

    fn format_result(&self) -> Vec<Line<'static>> {
        if let Some(result) = &self.result {
            let mut lines = vec![Line::from(Span::styled(
                "  Result:",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ))];

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

impl Component for ToolCallComponent {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Arc<Config>) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                Ok(Some(Action::ToggleToolCall(self.tool_call.id.clone())))
            }
            _ => Ok(None),
        }
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            Action::Render => {}
            Action::ToggleToolCall(id) if id == self.tool_call.id => {
                self.toggle_expanded();
            }
            Action::UpdateToolCallState { id, state } if id == self.tool_call.id => {
                self.set_state(state);
            }
            Action::UpdateToolCallResult { id, result } if id == self.tool_call.id => {
                self.set_result(result);
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let (symbol, color) = self.get_state_symbol_and_color();
        let expand_symbol = if self.expanded { "▼" } else { "▶" };

        let mut content = vec![Line::from(vec![
            Span::styled(symbol.to_string(), Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(expand_symbol, Style::default().fg(Color::Gray)),
            Span::raw(" "),
            Span::styled(
                "Tool Call:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                self.tool_call.name.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])];

        if !self.tool_call.id.is_empty() {
            content.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("ID: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    self.tool_call.id.clone(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        content.extend(self.format_arguments());
        content.extend(self.format_result());

        let text = Text::from(content);
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
        Ok(())
    }
}
