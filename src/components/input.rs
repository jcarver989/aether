use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{action::Action, config::Config};

#[derive(Debug, Clone)]
pub struct InputState {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    fn insert_char(&mut self, ch: char) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            if self.cursor_col <= line.len() {
                line.insert(self.cursor_col, ch);
                self.cursor_col += 1;
            }
        }
    }

    fn insert_newline(&mut self) {
        if self.cursor_line < self.lines.len() {
            let current_line = self.lines[self.cursor_line].clone();
            let (left, right) = current_line.split_at(self.cursor_col);

            self.lines[self.cursor_line] = left.to_string();
            self.lines.insert(self.cursor_line + 1, right.to_string());

            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    fn delete_char(&mut self) {
        if self.cursor_line < self.lines.len() {
            let line = &mut self.lines[self.cursor_line];
            if self.cursor_col > 0 && self.cursor_col <= line.len() {
                line.remove(self.cursor_col - 1);
                self.cursor_col -= 1;
            } else if self.cursor_col == 0 && self.cursor_line > 0 {
                let current_line = self.lines.remove(self.cursor_line);
                self.cursor_line -= 1;
                self.cursor_col = self.lines[self.cursor_line].len();
                self.lines[self.cursor_line].push_str(&current_line);
            }
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self
                .lines
                .get(self.cursor_line)
                .map(|l| l.len())
                .unwrap_or(0);
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor_line < self.lines.len() {
            let line_len = self.lines[self.cursor_line].len();
            if self.cursor_col < line_len {
                self.cursor_col += 1;
            } else if self.cursor_line + 1 < self.lines.len() {
                self.cursor_line += 1;
                self.cursor_col = 0;
            }
        }
    }

    fn move_cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            let line_len = self
                .lines
                .get(self.cursor_line)
                .map(|l| l.len())
                .unwrap_or(0);
            self.cursor_col = self.cursor_col.min(line_len);
        }
    }

    fn move_cursor_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            let line_len = self
                .lines
                .get(self.cursor_line)
                .map(|l| l.len())
                .unwrap_or(0);
            self.cursor_col = self.cursor_col.min(line_len);
        }
    }

    fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Input {
    state: InputState,
    placeholder: String,
    show_cursor: bool,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
    pub fn new() -> Self {
        Self {
            state: InputState::new(),
            placeholder: "Type your message...".to_string(),
            show_cursor: true,
            command_tx: None,
            config: Config::default(),
        }
    }

    fn format_lines(&self) -> Vec<Line<'static>> {
        if self.state.is_empty() && !self.placeholder.is_empty() {
            return vec![Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Green)),
                Span::styled(
                    self.placeholder.clone(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ])];
        }

        let mut lines = Vec::new();

        for (line_idx, line_content) in self.state.lines.iter().enumerate() {
            let mut spans = vec![if line_idx == 0 {
                Span::styled("> ", Style::default().fg(Color::Green))
            } else {
                Span::raw("  ")
            }];

            if line_idx == self.state.cursor_line && self.show_cursor {
                let (before_cursor, after_cursor) = line_content.split_at(self.state.cursor_col);

                if !before_cursor.is_empty() {
                    spans.push(Span::raw(before_cursor.to_string()));
                }

                let cursor_char = after_cursor.chars().next().unwrap_or(' ');
                spans.push(Span::styled(
                    cursor_char.to_string(),
                    Style::default().bg(Color::White).fg(Color::Black),
                ));

                if after_cursor.len() > 1 {
                    spans.push(Span::raw(after_cursor.chars().skip(1).collect::<String>()));
                }
            } else {
                spans.push(Span::raw(line_content.clone()));
            }

            lines.push(Line::from(spans));
        }

        if self.state.lines.len() == 1 && self.state.lines[0].is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Enter to submit, Shift+Enter for new line, Ctrl+C to cancel",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )]));
        } else if !self.state.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                format!("{} lines | Enter to submit", self.state.lines.len()),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )]));
        }

        lines
    }
}

impl Component for Input {
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
            KeyCode::Char(c) => {
                self.state.insert_char(c);
                Ok(None)
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.state.insert_newline();
                Ok(None)
            }
            KeyCode::Enter => {
                if !self.state.is_empty() {
                    let message = self.state.to_string();
                    self.state.clear();
                    Ok(Some(Action::SubmitMessage(message)))
                } else {
                    Ok(None)
                }
            }
            KeyCode::Backspace => {
                self.state.delete_char();
                Ok(None)
            }
            KeyCode::Left => {
                self.state.move_cursor_left();
                Ok(None)
            }
            KeyCode::Right => {
                self.state.move_cursor_right();
                Ok(None)
            }
            KeyCode::Up => {
                self.state.move_cursor_up();
                Ok(None)
            }
            KeyCode::Down => {
                self.state.move_cursor_down();
                Ok(None)
            }
            KeyCode::Esc => {
                self.state.clear();
                Ok(Some(Action::ClearInput))
            }
            _ => Ok(None),
        }
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            Action::Render => {}
            Action::ClearInput => {
                self.state.clear();
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let content = self.format_lines();
        let text = Text::from(content);

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Input"))
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
        Ok(())
    }
}
