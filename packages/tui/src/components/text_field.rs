use crossterm::event::KeyCode;

use crate::components::{Component, Event, ViewContext};
use crate::line::Line;

/// Single-line text input with cursor tracking and navigation.
pub struct TextField {
    pub value: String,
    cursor_pos: usize,
}

impl TextField {
    pub fn new(value: String) -> Self {
        let cursor_pos = value.len();
        Self { value, cursor_pos }
    }

    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    pub fn set_cursor_pos(&mut self, pos: usize) {
        self.cursor_pos = pos.min(self.value.len());
    }

    pub fn insert_at_cursor(&mut self, c: char) {
        self.value.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn insert_str_at_cursor(&mut self, s: &str) {
        self.value.insert_str(self.cursor_pos, s);
        self.cursor_pos += s.len();
    }

    pub fn delete_before_cursor(&mut self) -> bool {
        let Some((prev, _)) = self.value[..self.cursor_pos].char_indices().next_back() else {
            return false;
        };
        self.value.drain(prev..self.cursor_pos);
        self.cursor_pos = prev;
        true
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_pos = self.value[..self.cursor_pos]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
    }

    pub fn move_cursor_right(&mut self) {
        if let Some(c) = self.value[self.cursor_pos..].chars().next() {
            self.cursor_pos += c.len_utf8();
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.value.len();
    }

    pub fn set_value(&mut self, value: String) {
        self.cursor_pos = value.len();
        self.value = value;
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor_pos = 0;
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::String(self.value.clone())
    }

    pub fn render_field(&self, context: &ViewContext, focused: bool) -> Vec<Line> {
        let mut line = Line::new(&self.value);
        if focused {
            line.push_styled("▏", context.theme.primary());
        }
        vec![line]
    }
}

impl Component for TextField {
    type Message = ();

    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Char(c) => {
                    self.insert_at_cursor(c);
                    Some(vec![])
                }
                KeyCode::Backspace => {
                    self.delete_before_cursor();
                    Some(vec![])
                }
                KeyCode::Left => {
                    self.move_cursor_left();
                    Some(vec![])
                }
                KeyCode::Right => {
                    self.move_cursor_right();
                    Some(vec![])
                }
                KeyCode::Home => {
                    self.move_cursor_home();
                    Some(vec![])
                }
                KeyCode::End => {
                    self.move_cursor_end();
                    Some(vec![])
                }
                _ => None,
            },
            Event::Paste(text) => {
                self.insert_str_at_cursor(text);
                Some(vec![])
            }
            _ => None,
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        self.render_field(context, true)
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
    fn typing_appends_characters() {
        let mut field = TextField::new(String::new());
        field.on_event(&Event::Key(key(KeyCode::Char('h'))));
        field.on_event(&Event::Key(key(KeyCode::Char('i'))));
        assert_eq!(field.value, "hi");
    }

    #[test]
    fn backspace_removes_last_character() {
        let mut field = TextField::new("abc".to_string());
        field.on_event(&Event::Key(key(KeyCode::Backspace)));
        assert_eq!(field.value, "ab");
    }

    #[test]
    fn backspace_on_empty_is_no_op() {
        let mut field = TextField::new(String::new());
        field.on_event(&Event::Key(key(KeyCode::Backspace)));
        assert_eq!(field.value, "");
    }

    #[test]
    fn to_json_returns_string_value() {
        let field = TextField::new("hello".to_string());
        assert_eq!(field.to_json(), serde_json::json!("hello"));
    }

    #[test]
    fn unhandled_keys_are_ignored() {
        let mut field = TextField::new(String::new());
        let outcome = field.on_event(&Event::Key(key(KeyCode::Up)));
        assert!(outcome.is_none());
    }

    #[test]
    fn paste_appends_text() {
        let mut field = TextField::new(String::new());
        let outcome = field.on_event(&Event::Paste("hello".to_string()));
        assert!(outcome.is_some());
        assert_eq!(field.value, "hello");
    }

    #[test]
    fn cursor_starts_at_end() {
        let field = TextField::new("hello".to_string());
        assert_eq!(field.cursor_pos(), 5);
    }

    #[test]
    fn left_moves_cursor_back() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(key(KeyCode::Left)));
        assert_eq!(field.cursor_pos(), 4);
    }

    #[test]
    fn right_at_end_stays() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(key(KeyCode::Right)));
        assert_eq!(field.cursor_pos(), 5);
    }

    #[test]
    fn left_at_start_stays() {
        let mut field = TextField::new(String::new());
        field.on_event(&Event::Key(key(KeyCode::Left)));
        assert_eq!(field.cursor_pos(), 0);
    }

    #[test]
    fn home_moves_to_start() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(key(KeyCode::Home)));
        assert_eq!(field.cursor_pos(), 0);
    }

    #[test]
    fn end_moves_to_end() {
        let mut field = TextField::new("hello".to_string());
        field.move_cursor_home();
        field.on_event(&Event::Key(key(KeyCode::End)));
        assert_eq!(field.cursor_pos(), 5);
    }

    #[test]
    fn insert_at_middle() {
        let mut field = TextField::new("hllo".to_string());
        field.set_cursor_pos(1);
        field.on_event(&Event::Key(key(KeyCode::Char('e'))));
        assert_eq!(field.value, "hello");
        assert_eq!(field.cursor_pos(), 2);
    }

    #[test]
    fn backspace_at_middle() {
        let mut field = TextField::new("hello".to_string());
        field.set_cursor_pos(3);
        field.on_event(&Event::Key(key(KeyCode::Backspace)));
        assert_eq!(field.value, "helo");
        assert_eq!(field.cursor_pos(), 2);
    }

    #[test]
    fn paste_at_cursor() {
        let mut field = TextField::new("hd".to_string());
        field.set_cursor_pos(1);
        field.on_event(&Event::Paste("ello worl".to_string()));
        assert_eq!(field.value, "hello world");
        assert_eq!(field.cursor_pos(), 10);
    }

    #[test]
    fn multibyte_utf8_navigation() {
        let mut field = TextField::new("a中b".to_string());
        assert_eq!(field.cursor_pos(), 5);

        field.move_cursor_left();
        assert_eq!(field.cursor_pos(), 4);

        field.move_cursor_left();
        assert_eq!(field.cursor_pos(), 1);

        field.move_cursor_left();
        assert_eq!(field.cursor_pos(), 0);

        field.move_cursor_right();
        assert_eq!(field.cursor_pos(), 1);

        field.move_cursor_right();
        assert_eq!(field.cursor_pos(), 4);
    }

    #[test]
    fn set_value_moves_cursor_to_end() {
        let mut field = TextField::new(String::new());
        field.set_value("hello".to_string());
        assert_eq!(field.value, "hello");
        assert_eq!(field.cursor_pos(), 5);
    }

    #[test]
    fn clear_resets_cursor() {
        let mut field = TextField::new("hello".to_string());
        field.clear();
        assert_eq!(field.value, "");
        assert_eq!(field.cursor_pos(), 0);
    }
}
