use crate::tui::{HandlesInput, InputOutcome};
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

pub struct TextInput {
    buffer: String,
    cursor_pos: usize,
    mentions: Vec<SelectedFileMention>,
}

pub enum TextInputAction {
    Submit,
    OpenCommandPicker,
    OpenFilePicker,
}

#[derive(Debug, Clone)]
pub struct SelectedFileMention {
    pub mention: String,
    pub path: PathBuf,
    pub display_name: String,
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor_pos: 0,
            mentions: Vec::new(),
        }
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Returns the visual cursor index, accounting for an active file picker
    /// whose query extends beyond the `@` trigger character.
    pub fn cursor_index(&self, picker_query_len: Option<usize>) -> usize {
        if let Some(query_len) = picker_query_len {
            let at_pos = self.active_mention_start().unwrap_or(self.buffer.len());
            at_pos + 1 + query_len
        } else {
            self.cursor_pos
        }
    }

    pub fn take_mentions(&mut self) -> Vec<SelectedFileMention> {
        std::mem::take(&mut self.mentions)
    }

    pub fn set_input(&mut self, s: String) {
        self.cursor_pos = s.len();
        self.buffer = s;
    }

    pub fn clear(&mut self) {
        self.set_input(String::new());
    }

    pub fn insert_paste(&mut self, text: &str) {
        for c in text.chars() {
            if !c.is_control() {
                self.insert_char_at_cursor(c);
            }
        }
    }

    pub fn insert_char_at_cursor(&mut self, c: char) {
        self.buffer.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_char_before_cursor(&mut self) -> bool {
        let Some((prev, _)) = self.buffer[..self.cursor_pos].char_indices().next_back() else {
            return false;
        };
        self.buffer.drain(prev..self.cursor_pos);
        self.cursor_pos = prev;
        true
    }

    pub fn apply_file_selection(&mut self, path: PathBuf, display_name: String) {
        let mention = format!("@{display_name}");
        self.mentions.push(SelectedFileMention {
            mention: mention.clone(),
            path,
            display_name,
        });

        if let Some(at_pos) = self.active_mention_start() {
            let mut s = self.buffer[..at_pos].to_string();
            s.push_str(&mention);
            s.push(' ');
            self.set_input(s);
        }
    }

    fn active_mention_start(&self) -> Option<usize> {
        mention_start(&self.buffer)
    }

    fn move_cursor_left(&mut self) {
        self.cursor_pos = self.buffer[..self.cursor_pos]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
    }

    fn move_cursor_right(&mut self) {
        if let Some(c) = self.buffer[self.cursor_pos..].chars().next() {
            self.cursor_pos += c.len_utf8();
        }
    }

    fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    fn move_cursor_end(&mut self) {
        self.cursor_pos = self.buffer.len();
    }
}

impl HandlesInput for TextInput {
    type Action = TextInputAction;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action> {
        match key_event.code {
            KeyCode::Left => {
                self.move_cursor_left();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Right => {
                self.move_cursor_right();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Home => {
                self.move_cursor_home();
                InputOutcome::consumed_and_render()
            }
            KeyCode::End => {
                self.move_cursor_end();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Char('/') if self.buffer.is_empty() => {
                self.insert_char_at_cursor('/');
                InputOutcome::action_and_render(TextInputAction::OpenCommandPicker)
            }
            KeyCode::Char('@') => {
                self.insert_char_at_cursor('@');
                InputOutcome::action_and_render(TextInputAction::OpenFilePicker)
            }
            KeyCode::Char(c) => {
                self.insert_char_at_cursor(c);
                InputOutcome::consumed_and_render()
            }
            KeyCode::Backspace => {
                if self.delete_char_before_cursor() {
                    InputOutcome::consumed_and_render()
                } else {
                    InputOutcome::consumed()
                }
            }
            KeyCode::Enter => InputOutcome::action_and_render(TextInputAction::Submit),
            _ => InputOutcome::ignored(),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn left_arrow_moves_cursor_back_one_char() {
        let mut input = TextInput::new();
        input.set_input("hello".to_string());

        input.handle_key(key(KeyCode::Left));

        assert_eq!(input.cursor_index(None), 4);
    }

    #[test]
    fn right_arrow_moves_cursor_forward_one_char() {
        let mut input = TextInput::new();
        input.buffer = "hello".to_string();
        input.cursor_pos = 2;

        input.handle_key(key(KeyCode::Right));

        assert_eq!(input.cursor_index(None), 3);
    }

    #[test]
    fn left_at_start_stays_at_zero() {
        let mut input = TextInput::new();
        input.buffer = "hello".to_string();
        input.cursor_pos = 0;

        input.handle_key(key(KeyCode::Left));

        assert_eq!(input.cursor_index(None), 0);
    }

    #[test]
    fn right_at_end_stays_at_end() {
        let mut input = TextInput::new();
        input.set_input("hello".to_string());

        input.handle_key(key(KeyCode::Right));

        assert_eq!(input.cursor_index(None), 5);
    }

    #[test]
    fn home_moves_to_start() {
        let mut input = TextInput::new();
        input.buffer = "hello".to_string();
        input.cursor_pos = 3;

        input.handle_key(key(KeyCode::Home));

        assert_eq!(input.cursor_index(None), 0);
    }

    #[test]
    fn end_moves_to_end() {
        let mut input = TextInput::new();
        input.buffer = "hello".to_string();
        input.cursor_pos = 1;

        input.handle_key(key(KeyCode::End));

        assert_eq!(input.cursor_index(None), 5);
    }

    #[test]
    fn typing_inserts_at_cursor_position() {
        let mut input = TextInput::new();
        input.buffer = "hllo".to_string();
        input.cursor_pos = 1;

        input.handle_key(key(KeyCode::Char('e')));

        assert_eq!(input.buffer, "hello");
        assert_eq!(input.cursor_index(None), 2);
    }

    #[test]
    fn backspace_at_cursor_middle_deletes_correct_char() {
        let mut input = TextInput::new();
        input.buffer = "hello".to_string();
        input.cursor_pos = 3;

        input.handle_key(key(KeyCode::Backspace));

        assert_eq!(input.buffer, "helo");
        assert_eq!(input.cursor_index(None), 2);
    }

    #[test]
    fn backspace_at_start_does_nothing() {
        let mut input = TextInput::new();
        input.buffer = "hello".to_string();
        input.cursor_pos = 0;

        let outcome = input.handle_key(key(KeyCode::Backspace));

        assert!(outcome.consumed);
        assert!(!outcome.needs_render);
        assert_eq!(input.buffer, "hello");
        assert_eq!(input.cursor_index(None), 0);
    }

    #[test]
    fn multibyte_utf8_cursor_navigation() {
        let mut input = TextInput::new();
        // "a中b" — 'a' is 1 byte, '中' is 3 bytes, 'b' is 1 byte = 5 bytes total
        input.set_input("a中b".to_string());

        input.handle_key(key(KeyCode::Left));
        assert_eq!(input.cursor_index(None), 4); // before 'b'

        input.handle_key(key(KeyCode::Left));
        assert_eq!(input.cursor_index(None), 1); // before '中'

        input.handle_key(key(KeyCode::Left));
        assert_eq!(input.cursor_index(None), 0); // before 'a'

        input.handle_key(key(KeyCode::Right));
        assert_eq!(input.cursor_index(None), 1); // after 'a'

        input.handle_key(key(KeyCode::Right));
        assert_eq!(input.cursor_index(None), 4); // after '中'
    }

    #[test]
    fn paste_inserts_at_cursor_position() {
        let mut input = TextInput::new();
        input.buffer = "hd".to_string();
        input.cursor_pos = 1;

        input.insert_paste("ello worl");

        assert_eq!(input.buffer, "hello world");
        assert_eq!(input.cursor_index(None), 10);
    }

    #[test]
    fn slash_on_empty_returns_open_command_picker() {
        let mut input = TextInput::new();

        let outcome = input.handle_key(key(KeyCode::Char('/')));

        assert!(outcome.consumed);
        assert!(matches!(
            outcome.action,
            Some(TextInputAction::OpenCommandPicker)
        ));
        assert_eq!(input.buffer, "/");
    }

    #[test]
    fn at_sign_returns_open_file_picker() {
        let mut input = TextInput::new();

        let outcome = input.handle_key(key(KeyCode::Char('@')));

        assert!(outcome.consumed);
        assert!(matches!(
            outcome.action,
            Some(TextInputAction::OpenFilePicker)
        ));
        assert_eq!(input.buffer, "@");
    }

    #[test]
    fn enter_returns_submit() {
        let mut input = TextInput::new();
        input.set_input("hello".to_string());

        let outcome = input.handle_key(key(KeyCode::Enter));

        assert!(outcome.consumed);
        assert!(matches!(outcome.action, Some(TextInputAction::Submit)));
    }

    #[test]
    fn file_selection_updates_mentions_and_buffer() {
        let mut input = TextInput::new();
        input.set_input("@fo".to_string());

        input.apply_file_selection(PathBuf::from("foo.rs"), "foo.rs".to_string());

        assert_eq!(input.buffer, "@foo.rs ");
        assert_eq!(input.mentions.len(), 1);
        assert_eq!(input.mentions[0].mention, "@foo.rs");
    }

    #[test]
    fn cursor_index_without_picker() {
        let mut input = TextInput::new();
        input.buffer = "hello".to_string();
        input.cursor_pos = 3;

        assert_eq!(input.cursor_index(None), 3);
    }

    #[test]
    fn cursor_index_with_picker_query() {
        let mut input = TextInput::new();
        input.set_input("@fo".to_string());

        // Picker has 2-char query ("fo"), @ is at position 0
        assert_eq!(input.cursor_index(Some(2)), 3); // 0 + 1 + 2
    }

    #[test]
    fn clear_resets_buffer_and_cursor() {
        let mut input = TextInput::new();
        input.set_input("hello".to_string());

        input.clear();

        assert_eq!(input.buffer, "");
        assert_eq!(input.cursor_index(None), 0);
    }
}
