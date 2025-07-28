use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
use crate::{action::{Action, CursorDirection}, config::Config};

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


    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
    }
}

impl std::fmt::Display for InputState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.lines.join("\n"))
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
    config: Arc<Config>,
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
            config: Arc::new(Config::default()),
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

    fn register_config_handler(&mut self, config: Arc<Config>) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        // Only process key press events, ignore release and repeat events
        if key.kind != KeyEventKind::Press {
            return Ok(None);
        }

        match key.code {
            KeyCode::Char(c) => Ok(Some(Action::InsertChar(c))),
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                Ok(Some(Action::InsertNewline))
            }
            KeyCode::Enter => {
                if !self.state.is_empty() {
                    let message = self.state.to_string();
                    Ok(Some(Action::SubmitMessage(message)))
                } else {
                    Ok(None)
                }
            }
            KeyCode::Backspace => Ok(Some(Action::DeleteChar)),
            KeyCode::Left => Ok(Some(Action::MoveCursor(CursorDirection::Left))),
            KeyCode::Right => Ok(Some(Action::MoveCursor(CursorDirection::Right))),
            KeyCode::Up => Ok(Some(Action::MoveCursor(CursorDirection::Up))),
            KeyCode::Down => Ok(Some(Action::MoveCursor(CursorDirection::Down))),
            KeyCode::Esc => Ok(Some(Action::ClearInput)),
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
            Action::InsertChar(c) => {
                self.state.insert_char(c);
            }
            Action::InsertNewline => {
                self.state.insert_newline();
            }
            Action::DeleteChar => {
                self.state.delete_char();
            }
            Action::MoveCursor(direction) => {
                match direction {
                    CursorDirection::Left => self.state.move_cursor_left(),
                    CursorDirection::Right => self.state.move_cursor_right(),
                    CursorDirection::Up => self.state.move_cursor_up(),
                    CursorDirection::Down => self.state.move_cursor_down(),
                }
            }
            Action::SubmitMessage(_) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Size};

    // Test buffer dimensions for input testing
    const TEST_BUFFER_WIDTH: u16 = 60;
    const TEST_BUFFER_HEIGHT: u16 = 10;

    /// Helper function to extract text content from a buffer range
    fn extract_buffer_text(buffer: &Buffer, start: usize, end: usize) -> String {
        buffer.content()[start..end]
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    /// Helper function to extract a single line from the buffer
    fn extract_buffer_line(buffer: &Buffer, line: usize, width: usize) -> String {
        let start = line * width;
        let end = start + width;
        extract_buffer_text(buffer, start, end)
    }

    /// Helper function to create terminal and draw input component
    fn draw_input_component(input: &mut Input, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("Failed to create test terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                input.draw(frame, area).expect("Failed to draw input component");
            })
            .expect("Failed to draw terminal frame");

        terminal.backend().buffer().clone()
    }

    #[test]
    fn test_input_state_new() {
        let state = InputState::new();
        assert_eq!(state.lines, vec![String::new()]);
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn test_input_state_default() {
        let state = InputState::default();
        assert_eq!(state.lines, vec![String::new()]);
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn test_input_state_clone() {
        let mut state1 = InputState::new();
        state1.insert_char('a');
        let state2 = state1.clone();
        assert_eq!(state1.lines, state2.lines);
        assert_eq!(state1.cursor_line, state2.cursor_line);
        assert_eq!(state1.cursor_col, state2.cursor_col);
    }

    #[test]
    fn test_input_state_insert_char() {
        let mut state = InputState::new();

        // Insert characters at beginning
        state.insert_char('h');
        state.insert_char('e');
        state.insert_char('l');
        state.insert_char('l');
        state.insert_char('o');

        assert_eq!(state.lines[0], "hello");
        assert_eq!(state.cursor_col, 5);

        // Insert character in middle
        state.cursor_col = 2;
        state.insert_char('X');
        assert_eq!(state.lines[0], "heXllo");
        assert_eq!(state.cursor_col, 3);
    }

    #[test]
    fn test_input_state_insert_newline() {
        let mut state = InputState::new();

        // Insert text then newline
        state.insert_char('h');
        state.insert_char('e');
        state.insert_char('l');
        state.insert_char('l');
        state.insert_char('o');
        state.insert_newline();

        assert_eq!(state.lines.len(), 2);
        assert_eq!(state.lines[0], "hello");
        assert_eq!(state.lines[1], "");
        assert_eq!(state.cursor_line, 1);
        assert_eq!(state.cursor_col, 0);

        // Insert newline in middle of text
        state.lines[0] = "hello world".to_string();
        state.cursor_line = 0;
        state.cursor_col = 5;
        state.insert_newline();

        assert_eq!(state.lines.len(), 3);
        assert_eq!(state.lines[0], "hello");
        assert_eq!(state.lines[1], " world");
        assert_eq!(state.cursor_line, 1);
        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn test_input_state_delete_char() {
        let mut state = InputState::new();

        // Setup multi-line text
        state.lines = vec!["hello".to_string(), "world".to_string()];
        state.cursor_line = 0;
        state.cursor_col = 5;

        // Delete from end of line
        state.delete_char();
        assert_eq!(state.lines[0], "hell");
        assert_eq!(state.cursor_col, 4);

        // Delete from middle
        state.cursor_col = 2;
        state.delete_char();
        assert_eq!(state.lines[0], "hll");
        assert_eq!(state.cursor_col, 1);

        // Delete at beginning of line (should merge with previous line)
        state.cursor_line = 1;
        state.cursor_col = 0;
        state.delete_char();

        assert_eq!(state.lines.len(), 1);
        assert_eq!(state.lines[0], "hllworld");
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 3);
    }

    #[test]
    fn test_input_state_move_cursor_left() {
        let mut state = InputState::new();
        state.lines = vec!["hello".to_string(), "world".to_string()];
        state.cursor_line = 1;
        state.cursor_col = 3;

        // Move left within line
        state.move_cursor_left();
        assert_eq!(state.cursor_line, 1);
        assert_eq!(state.cursor_col, 2);

        // Move to beginning of line
        state.cursor_col = 0;
        state.move_cursor_left();
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 5);

        // Can't move left from (0, 0)
        state.cursor_line = 0;
        state.cursor_col = 0;
        state.move_cursor_left();
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn test_input_state_move_cursor_right() {
        let mut state = InputState::new();
        state.lines = vec!["hello".to_string(), "world".to_string()];
        state.cursor_line = 0;
        state.cursor_col = 3;

        // Move right within line
        state.move_cursor_right();
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 4);

        // Move to next line from end of line
        state.cursor_col = 5;
        state.move_cursor_right();
        assert_eq!(state.cursor_line, 1);
        assert_eq!(state.cursor_col, 0);

        // Can't move right from end of last line
        state.cursor_line = 1;
        state.cursor_col = 5;
        state.move_cursor_right();
        assert_eq!(state.cursor_line, 1);
        assert_eq!(state.cursor_col, 5);
    }

    #[test]
    fn test_input_state_move_cursor_up() {
        let mut state = InputState::new();
        state.lines = vec!["hello world".to_string(), "short".to_string()];
        state.cursor_line = 1;
        state.cursor_col = 3;

        // Move up
        state.move_cursor_up();
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 3);

        // Move up with cursor beyond line length
        state.cursor_line = 1;
        state.cursor_col = 5;
        state.move_cursor_up();
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 5); // Clamps to line length

        // Can't move up from first line
        state.cursor_line = 0;
        state.move_cursor_up();
        assert_eq!(state.cursor_line, 0);
    }

    #[test]
    fn test_input_state_move_cursor_down() {
        let mut state = InputState::new();
        state.lines = vec!["hello world".to_string(), "short".to_string()];
        state.cursor_line = 0;
        state.cursor_col = 8;

        // Move down with cursor beyond line length
        state.move_cursor_down();
        assert_eq!(state.cursor_line, 1);
        assert_eq!(state.cursor_col, 5); // Clamps to line length

        // Can't move down from last line
        state.move_cursor_down();
        assert_eq!(state.cursor_line, 1);
        assert_eq!(state.cursor_col, 5);
    }

    #[test]
    fn test_input_state_to_string() {
        let mut state = InputState::new();
        assert_eq!(state.to_string(), "");

        state.lines = vec!["hello".to_string(), "world".to_string()];
        assert_eq!(state.to_string(), "hello\nworld");

        state.lines = vec!["single".to_string()];
        assert_eq!(state.to_string(), "single");
    }

    #[test]
    fn test_input_state_is_empty() {
        let mut state = InputState::new();
        assert!(state.is_empty());

        state.insert_char('a');
        assert!(!state.is_empty());

        state.clear();
        assert!(state.is_empty());

        state.lines = vec!["".to_string(), "content".to_string()];
        assert!(!state.is_empty());
    }

    #[test]
    fn test_input_state_clear() {
        let mut state = InputState::new();
        state.lines = vec!["hello".to_string(), "world".to_string()];
        state.cursor_line = 1;
        state.cursor_col = 3;

        state.clear();

        assert_eq!(state.lines, vec![String::new()]);
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn test_input_new() {
        let input = Input::new();
        assert_eq!(input.state.lines, vec![String::new()]);
        assert_eq!(input.state.cursor_line, 0);
        assert_eq!(input.state.cursor_col, 0);
        assert_eq!(input.placeholder, "Type your message...");
        assert!(input.show_cursor);
        assert!(input.command_tx.is_none());
    }

    #[test]
    fn test_input_default() {
        let input = Input::default();
        assert_eq!(input.state.lines, vec![String::new()]);
        assert_eq!(input.state.cursor_line, 0);
        assert_eq!(input.state.cursor_col, 0);
        assert_eq!(input.placeholder, "Type your message...");
        assert!(input.show_cursor);
        assert!(input.command_tx.is_none());
    }

    #[test]
    fn test_handle_key_event_char() {
        let mut input = Input::new();

        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::InsertChar('a')));
        input.update(Action::InsertChar('a')).expect("Failed to update with InsertChar('a')");
        assert_eq!(input.state.lines[0], "a");
        assert_eq!(input.state.cursor_col, 1);

        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::InsertChar('b')));
        input.update(Action::InsertChar('b')).expect("Failed to update with InsertChar('b')");
        assert_eq!(input.state.lines[0], "ab");
        assert_eq!(input.state.cursor_col, 2);
    }

    #[test]
    fn test_handle_key_event_enter() {
        let mut input = Input::new();

        // Enter on empty input should return None
        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, None);

        // Add some text via actions
        input.update(Action::InsertChar('h')).expect("Failed to update with InsertChar('h')");
        input.update(Action::InsertChar('i')).expect("Failed to update with InsertChar('i')");

        // Enter with content should submit and clear
        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::SubmitMessage("hi".to_string())));
        input.update(Action::SubmitMessage("hi".to_string())).expect("Failed to update with SubmitMessage");
        assert!(input.state.is_empty());
    }

    #[test]
    fn test_handle_key_event_shift_enter() {
        let mut input = Input::new();
        input.update(Action::InsertChar('h')).expect("Failed to update with InsertChar('h')");
        input.update(Action::InsertChar('i')).expect("Failed to update with InsertChar('i')");

        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::InsertNewline));
        input.update(Action::InsertNewline).expect("Failed to update with InsertNewline");
        assert_eq!(input.state.lines.len(), 2);
        assert_eq!(input.state.lines[0], "hi");
        assert_eq!(input.state.lines[1], "");
        assert_eq!(input.state.cursor_line, 1);
        assert_eq!(input.state.cursor_col, 0);
    }

    #[test]
    fn test_handle_key_event_backspace() {
        let mut input = Input::new();
        input.update(Action::InsertChar('a')).expect("Failed to update with InsertChar('a')");
        input.update(Action::InsertChar('b')).expect("Failed to update with InsertChar('b')");

        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::DeleteChar));
        input.update(Action::DeleteChar).expect("Failed to update with DeleteChar");
        assert_eq!(input.state.lines[0], "a");
        assert_eq!(input.state.cursor_col, 1);
    }

    #[test]
    fn test_handle_key_event_arrow_keys() {
        let mut input = Input::new();
        input.state.lines = vec!["hello".to_string(), "world".to_string()];
        input.state.cursor_line = 0;
        input.state.cursor_col = 2;

        // Test Left
        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::MoveCursor(CursorDirection::Left)));
        input.update(Action::MoveCursor(CursorDirection::Left)).expect("Failed to update with MoveCursor(Left)");
        assert_eq!(input.state.cursor_col, 1);

        // Test Right
        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::MoveCursor(CursorDirection::Right)));
        input.update(Action::MoveCursor(CursorDirection::Right)).expect("Failed to update with MoveCursor(Right)");
        assert_eq!(input.state.cursor_col, 2);

        // Test Down
        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::MoveCursor(CursorDirection::Down)));
        input.update(Action::MoveCursor(CursorDirection::Down)).expect("Failed to update with MoveCursor(Down)");
        assert_eq!(input.state.cursor_line, 1);

        // Test Up
        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::MoveCursor(CursorDirection::Up)));
        input.update(Action::MoveCursor(CursorDirection::Up)).expect("Failed to update with MoveCursor(Up)");
        assert_eq!(input.state.cursor_line, 0);
    }

    #[test]
    fn test_handle_key_event_escape() {
        let mut input = Input::new();
        input.update(Action::InsertChar('t')).expect("Failed to update with InsertChar('t')");
        input.update(Action::InsertChar('e')).expect("Failed to update with InsertChar('e')");
        input.update(Action::InsertChar('s')).expect("Failed to update with InsertChar('s')");
        input.update(Action::InsertChar('t')).expect("Failed to update with InsertChar('t')");

        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, Some(Action::ClearInput));
        input.update(Action::ClearInput).expect("Failed to update with ClearInput");
        assert!(input.state.is_empty());
    }

    #[test]
    fn test_handle_key_event_unhandled() {
        let mut input = Input::new();

        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, None);

        let result = input
            .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
            .expect("Failed to handle key event");
        assert_eq!(result, None);
    }

    #[test]
    fn test_update_actions() {
        let mut input = Input::new();
        input.state.insert_char('t');
        input.state.insert_char('e');
        input.state.insert_char('s');
        input.state.insert_char('t');

        // Test Tick action
        let result = input.update(Action::Tick).expect("Failed to update with Tick");
        assert_eq!(result, None);
        assert_eq!(input.state.lines[0], "test"); // Should be unchanged

        // Test Render action
        let result = input.update(Action::Render).expect("Failed to update with Render");
        assert_eq!(result, None);
        assert_eq!(input.state.lines[0], "test"); // Should be unchanged

        // Test ClearInput action
        let result = input.update(Action::ClearInput).expect("Failed to update with ClearInput");
        assert_eq!(result, None);
        assert!(input.state.is_empty());

        // Test unhandled action
        let result = input.update(Action::Quit).expect("Failed to update with Quit");
        assert_eq!(result, None);
    }

    #[test]
    fn test_draw_renders_input_content() {
        let mut input = Input::new();
        input.state.insert_char('H');
        input.state.insert_char('e');
        input.state.insert_char('l');
        input.state.insert_char('l');
        input.state.insert_char('o');

        let buffer = draw_input_component(&mut input, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find input text in buffer
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(
            all_content.contains("Hello"),
            "Should find 'Hello' in buffer"
        );
        assert!(
            all_content.contains("> "),
            "Should find prompt marker in buffer"
        );
    }

    #[test]
    fn test_draw_renders_multiline_content() {
        let mut input = Input::new();
        input.state.insert_char('L');
        input.state.insert_char('i');
        input.state.insert_char('n');
        input.state.insert_char('e');
        input.state.insert_char(' ');
        input.state.insert_char('1');
        input.state.insert_newline();
        input.state.insert_char('L');
        input.state.insert_char('i');
        input.state.insert_char('n');
        input.state.insert_char('e');
        input.state.insert_char(' ');
        input.state.insert_char('2');

        let buffer = draw_input_component(&mut input, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find both lines in buffer
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(
            all_content.contains("Line 1"),
            "Should find 'Line 1' in buffer"
        );
        assert!(
            all_content.contains("Line 2"),
            "Should find 'Line 2' in buffer"
        );
        assert!(
            all_content.contains("> "),
            "Should find prompt marker for first line"
        );

        // Should show line count
        assert!(
            all_content.contains("2 lines"),
            "Should show line count in help text"
        );
    }

    #[test]
    fn test_draw_shows_placeholder_when_empty() {
        let mut input = Input::new();
        let buffer = draw_input_component(&mut input, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find placeholder text when empty (not help text)
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(
            all_content.contains("Type your message..."),
            "Should find placeholder text"
        );
        assert!(all_content.contains("> "), "Should find prompt marker");

        // When empty with placeholder, should NOT show help text
        assert!(
            !all_content.contains("Enter to submit"),
            "Should not show help text when placeholder is shown"
        );
    }

    #[test]
    fn test_draw_shows_help_text_with_content() {
        let mut input = Input::new();

        // Add some content to see help text
        input.state.insert_char('t');
        input.state.insert_char('e');
        input.state.insert_char('s');
        input.state.insert_char('t');

        let buffer_with_content =
            draw_input_component(&mut input, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let content_with_text =
            extract_buffer_text(&buffer_with_content, 0, buffer_with_content.content().len());

        assert!(
            content_with_text.contains("1 lines"),
            "Should show line count"
        );
        assert!(
            content_with_text.contains("Enter to submit"),
            "Should show submit help"
        );
    }

    #[test]
    fn test_draw_shows_help_text_when_no_placeholder() {
        let mut input = Input::new();
        // Remove placeholder to see help text for empty input
        input.placeholder = String::new();

        let buffer = draw_input_component(&mut input, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        // Should find help text when there's no placeholder
        assert!(
            all_content.contains("Enter to submit"),
            "Should find help text about Enter"
        );
        assert!(
            all_content.contains("Shift+Enter"),
            "Should find help text about Shift+Enter"
        );
        assert!(
            all_content.contains("Ctrl+C"),
            "Should find help text about Ctrl+C"
        );
    }

    #[test]
    fn test_draw_renders_borders_and_title() {
        let mut input = Input::new();
        let buffer = draw_input_component(&mut input, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // First line should contain the top border with "Input" title
        let first_line = extract_buffer_line(&buffer, 0, TEST_BUFFER_WIDTH as usize);
        assert!(first_line.contains("┌"), "Should contain top-left corner");
        assert!(first_line.contains("Input"), "Should contain Input title");
        assert!(first_line.contains("┐"), "Should contain top-right corner");

        // Last line should contain the bottom border
        let last_line = extract_buffer_line(
            &buffer,
            (TEST_BUFFER_HEIGHT - 1) as usize,
            TEST_BUFFER_WIDTH as usize,
        );
        assert!(last_line.contains("└"), "Should contain bottom-left corner");
        assert!(
            last_line.contains("┘"),
            "Should contain bottom-right corner"
        );

        // Middle lines should have side borders
        for line_num in 1..(TEST_BUFFER_HEIGHT - 1) {
            let line = extract_buffer_line(&buffer, line_num as usize, TEST_BUFFER_WIDTH as usize);
            assert!(
                line.starts_with('│'),
                "Line {line_num} should start with left border"
            );
            assert!(
                line.ends_with('│'),
                "Line {line_num} should end with right border"
            );
        }
    }

    #[test]
    fn test_component_trait_methods() {
        let mut input = Input::new();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        // Test register_action_handler
        let result = input.register_action_handler(tx.clone());
        assert!(result.is_ok());
        assert!(input.command_tx.is_some());

        // Test register_config_handler
        let config = Arc::new(Config::default());
        let result = input.register_config_handler(config);
        assert!(result.is_ok());

        // Test init (default implementation)
        let size = Size::new(80, 24);
        let result = input.init(size);
        assert!(result.is_ok());

        // Test handle_events (default implementation)
        let result = input.handle_events(None);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should succeed"), None);

        // Test handle_mouse_event (default implementation)
        let mouse_event = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        };
        let result = input.handle_mouse_event(mouse_event);
        assert!(result.is_ok());
        assert_eq!(result.expect("Should succeed"), None);
    }

    #[test]
    fn test_draw_with_cursor_highlighting() {
        let mut input = Input::new();
        input.state.insert_char('t');
        input.state.insert_char('e');
        input.state.insert_char('s');
        input.state.insert_char('t');
        input.state.cursor_col = 2; // Position cursor at 's'

        let buffer = draw_input_component(&mut input, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);

        // Should find the text content
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());
        assert!(all_content.contains("test"), "Should find 'test' in buffer");

        // Cursor should be visible (though we can't easily test the visual highlighting in text)
        assert!(input.show_cursor, "Cursor should be visible");
    }

    #[test]
    fn test_complex_editing_scenario() {
        let mut input = Input::new();

        // Type "Hello World" via actions
        for ch in "Hello World".chars() {
            let action = input
                .handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .expect("Failed to handle key event").expect("Should return an action");
            input.update(action).expect("Failed to update input");
        }

        // Move cursor to position 5 (after "Hello")
        input.state.cursor_col = 5;

        // Insert newline
        let action = input
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT))
            .expect("Failed to handle key event").expect("Should return an action");
        input.update(action).expect("Failed to update input");

        // Type "Beautiful "
        for ch in "Beautiful ".chars() {
            let action = input
                .handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .expect("Failed to handle key event").expect("Should return an action");
            input.update(action).expect("Failed to update input");
        }

        let buffer = draw_input_component(&mut input, TEST_BUFFER_WIDTH, TEST_BUFFER_HEIGHT);
        let all_content = extract_buffer_text(&buffer, 0, buffer.content().len());

        // The result should be: "Hello" on first line, "Beautiful  World" on second line
        assert!(all_content.contains("Hello"), "Should contain 'Hello'");
        assert!(
            all_content.contains("Beautiful"),
            "Should contain 'Beautiful'"
        );
        assert!(all_content.contains("World"), "Should contain 'World'");
        assert!(
            all_content.contains("2 lines"),
            "Should show correct line count"
        );

        // Verify the actual line content structure
        assert_eq!(input.state.lines[0], "Hello");
        assert_eq!(input.state.lines[1], "Beautiful  World");
    }

    #[test]
    fn test_edge_case_cursor_positioning() {
        let mut input = Input::new();

        // Test cursor at very beginning
        assert_eq!(input.state.cursor_line, 0);
        assert_eq!(input.state.cursor_col, 0);

        // Try to move left (should stay at 0,0)
        let action = input
            .handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
            .expect("Failed to handle key event").expect("Should return an action");
        input.update(action).expect("Failed to update input");
        assert_eq!(input.state.cursor_line, 0);
        assert_eq!(input.state.cursor_col, 0);

        // Try to move up (should stay at 0,0)
        let action = input
            .handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            .expect("Failed to handle key event").expect("Should return an action");
        input.update(action).expect("Failed to update input");
        assert_eq!(input.state.cursor_line, 0);
        assert_eq!(input.state.cursor_col, 0);

        // Add some text and test end positioning
        for ch in "test".chars() {
            let action = input
                .handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
                .expect("Failed to handle key event").expect("Should return an action");
            input.update(action).expect("Failed to update input");
        }

        // Should be at end of line
        assert_eq!(input.state.cursor_col, 4);

        // Try to move right beyond end (should stay at end)
        let action = input
            .handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
            .expect("Failed to handle key event").expect("Should return an action");
        input.update(action).expect("Failed to update input");
        assert_eq!(input.state.cursor_col, 4);

        // Try to move down from single line (should stay on line 0)
        let action = input
            .handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .expect("Failed to handle key event").expect("Should return an action");
        input.update(action).expect("Failed to update input");
        assert_eq!(input.state.cursor_line, 0);
    }
}
