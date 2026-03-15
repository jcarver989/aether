use crossterm::event::{KeyCode, KeyModifiers};
use unicode_width::UnicodeWidthChar;

use crate::components::{Component, Event, ViewContext};
use crate::line::Line;
use crate::rendering::frame::Frame;
use crate::rendering::soft_wrap::display_width_text;

/// Single-line text input with cursor tracking and navigation.
pub struct TextField {
    pub value: String,
    cursor_pos: usize,
    content_width: usize,
}

impl TextField {
    pub fn new(value: String) -> Self {
        let cursor_pos = value.len();
        Self {
            value,
            cursor_pos,
            content_width: usize::MAX,
        }
    }

    pub fn set_content_width(&mut self, width: usize) {
        self.content_width = width.max(1);
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

    fn delete_after_cursor(&mut self) {
        if let Some(c) = self.value[self.cursor_pos..].chars().next() {
            self.value
                .drain(self.cursor_pos..self.cursor_pos + c.len_utf8());
        }
    }

    fn delete_word_backward(&mut self) {
        let end = self.cursor_pos;
        let start = self.word_start_backward();
        self.cursor_pos = start;
        self.value.drain(start..end);
    }

    fn word_end_forward(&mut self) {
        let len = self.value.len();
        while self.cursor_pos < len {
            let ch = self.value[self.cursor_pos..].chars().next().unwrap();
            if ch.is_whitespace() {
                break;
            }
            self.cursor_pos += ch.len_utf8();
        }
        while self.cursor_pos < len {
            let ch = self.value[self.cursor_pos..].chars().next().unwrap();
            if !ch.is_whitespace() {
                break;
            }
            self.cursor_pos += ch.len_utf8();
        }
    }

    fn move_cursor_up(&mut self, content_width: usize) {
        if content_width == 0 {
            return;
        }
        let cursor_width = self.display_width_up_to(self.cursor_pos);
        let row = cursor_width / content_width;
        if row == 0 {
            self.cursor_pos = 0;
        } else {
            let col = cursor_width % content_width;
            let target = (row - 1) * content_width + col;
            self.cursor_pos = self.byte_offset_for_display_width(target);
        }
    }

    fn move_cursor_down(&mut self, content_width: usize) {
        if content_width == 0 {
            return;
        }
        let cursor_width = self.display_width_up_to(self.cursor_pos);
        let total_width = self.display_width_up_to(self.value.len());
        let row = cursor_width / content_width;
        let max_row = total_width / content_width;
        if row >= max_row {
            self.cursor_pos = self.value.len();
        } else {
            let col = cursor_width % content_width;
            let target = ((row + 1) * content_width + col).min(total_width);
            self.cursor_pos = self.byte_offset_for_display_width(target);
        }
    }

    fn word_start_backward(&self) -> usize {
        let mut pos = self.cursor_pos;
        while pos > 0 {
            let (i, ch) = self.value[..pos].char_indices().next_back().unwrap();
            if !ch.is_whitespace() {
                break;
            }
            pos = i;
        }
        while pos > 0 {
            let (i, ch) = self.value[..pos].char_indices().next_back().unwrap();
            if ch.is_whitespace() {
                break;
            }
            pos = i;
        }
        pos
    }

    fn display_width_up_to(&self, byte_pos: usize) -> usize {
        display_width_text(&self.value[..byte_pos])
    }

    fn byte_offset_for_display_width(&self, target_width: usize) -> usize {
        let mut width = 0;
        for (i, ch) in self.value.char_indices() {
            if width >= target_width {
                return i;
            }
            width += UnicodeWidthChar::width(ch).unwrap_or(0);
        }
        self.value.len()
    }
}

impl Component for TextField {
    type Message = ();

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        match event {
            Event::Key(key) => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                match key.code {
                    KeyCode::Char('a') if ctrl => {
                        self.cursor_pos = 0;
                        Some(vec![])
                    }
                    KeyCode::Char('e') if ctrl => {
                        self.cursor_pos = self.value.len();
                        Some(vec![])
                    }
                    KeyCode::Char('w') if ctrl => {
                        self.delete_word_backward();
                        Some(vec![])
                    }
                    KeyCode::Char('u') if ctrl => {
                        self.value.drain(..self.cursor_pos);
                        self.cursor_pos = 0;
                        Some(vec![])
                    }
                    KeyCode::Char('k') if ctrl => {
                        self.value.truncate(self.cursor_pos);
                        Some(vec![])
                    }
                    KeyCode::Backspace if alt => {
                        self.delete_word_backward();
                        Some(vec![])
                    }
                    KeyCode::Left if alt || ctrl => {
                        self.cursor_pos = self.word_start_backward();
                        Some(vec![])
                    }
                    KeyCode::Right if alt || ctrl => {
                        self.word_end_forward();
                        Some(vec![])
                    }
                    KeyCode::Delete => {
                        self.delete_after_cursor();
                        Some(vec![])
                    }
                    KeyCode::Char(c) if !ctrl => {
                        self.insert_at_cursor(c);
                        Some(vec![])
                    }
                    KeyCode::Backspace => {
                        self.delete_before_cursor();
                        Some(vec![])
                    }
                    KeyCode::Left => {
                        self.cursor_pos = self.value[..self.cursor_pos]
                            .char_indices()
                            .next_back()
                            .map_or(0, |(i, _)| i);
                        Some(vec![])
                    }
                    KeyCode::Right => {
                        if let Some(c) = self.value[self.cursor_pos..].chars().next() {
                            self.cursor_pos += c.len_utf8();
                        }
                        Some(vec![])
                    }
                    KeyCode::Home => {
                        self.cursor_pos = 0;
                        Some(vec![])
                    }
                    KeyCode::End => {
                        self.cursor_pos = self.value.len();
                        Some(vec![])
                    }
                    KeyCode::Up => {
                        self.move_cursor_up(self.content_width);
                        Some(vec![])
                    }
                    KeyCode::Down => {
                        self.move_cursor_down(self.content_width);
                        Some(vec![])
                    }
                    _ => None,
                }
            }
            Event::Paste(text) => {
                self.insert_str_at_cursor(text);
                Some(vec![])
            }
            _ => None,
        }
    }

    fn render(&mut self, context: &ViewContext) -> Frame {
        Frame::new(self.render_field(context, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn alt(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::ALT)
    }

    #[tokio::test]
    async fn typing_appends_characters() {
        let mut field = TextField::new(String::new());
        field.on_event(&Event::Key(key(KeyCode::Char('h')))).await;
        field.on_event(&Event::Key(key(KeyCode::Char('i')))).await;
        assert_eq!(field.value, "hi");
    }

    #[tokio::test]
    async fn backspace_removes_last_character() {
        let mut field = TextField::new("abc".to_string());
        field.on_event(&Event::Key(key(KeyCode::Backspace))).await;
        assert_eq!(field.value, "ab");
    }

    #[tokio::test]
    async fn backspace_on_empty_is_no_op() {
        let mut field = TextField::new(String::new());
        field.on_event(&Event::Key(key(KeyCode::Backspace))).await;
        assert_eq!(field.value, "");
    }

    #[test]
    fn to_json_returns_string_value() {
        let field = TextField::new("hello".to_string());
        assert_eq!(field.to_json(), serde_json::json!("hello"));
    }

    #[tokio::test]
    async fn unhandled_keys_are_ignored() {
        let mut field = TextField::new(String::new());
        let outcome = field.on_event(&Event::Key(key(KeyCode::F(1)))).await;
        assert!(outcome.is_none());
    }

    #[tokio::test]
    async fn paste_appends_text() {
        let mut field = TextField::new(String::new());
        let outcome = field.on_event(&Event::Paste("hello".to_string())).await;
        assert!(outcome.is_some());
        assert_eq!(field.value, "hello");
    }

    #[test]
    fn cursor_starts_at_end() {
        let field = TextField::new("hello".to_string());
        assert_eq!(field.cursor_pos(), 5);
    }

    #[tokio::test]
    async fn left_moves_cursor_back() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(key(KeyCode::Left))).await;
        assert_eq!(field.cursor_pos(), 4);
    }

    #[tokio::test]
    async fn right_at_end_stays() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(key(KeyCode::Right))).await;
        assert_eq!(field.cursor_pos(), 5);
    }

    #[tokio::test]
    async fn left_at_start_stays() {
        let mut field = TextField::new(String::new());
        field.on_event(&Event::Key(key(KeyCode::Left))).await;
        assert_eq!(field.cursor_pos(), 0);
    }

    #[tokio::test]
    async fn home_moves_to_start() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(key(KeyCode::Home))).await;
        assert_eq!(field.cursor_pos(), 0);
    }

    #[tokio::test]
    async fn end_moves_to_end() {
        let mut field = TextField::new("hello".to_string());
        field.set_cursor_pos(0);
        field.on_event(&Event::Key(key(KeyCode::End))).await;
        assert_eq!(field.cursor_pos(), 5);
    }

    #[tokio::test]
    async fn insert_at_middle() {
        let mut field = TextField::new("hllo".to_string());
        field.set_cursor_pos(1);
        field.on_event(&Event::Key(key(KeyCode::Char('e')))).await;
        assert_eq!(field.value, "hello");
        assert_eq!(field.cursor_pos(), 2);
    }

    #[tokio::test]
    async fn backspace_at_middle() {
        let mut field = TextField::new("hello".to_string());
        field.set_cursor_pos(3);
        field.on_event(&Event::Key(key(KeyCode::Backspace))).await;
        assert_eq!(field.value, "helo");
        assert_eq!(field.cursor_pos(), 2);
    }

    #[tokio::test]
    async fn paste_at_cursor() {
        let mut field = TextField::new("hd".to_string());
        field.set_cursor_pos(1);
        field.on_event(&Event::Paste("ello worl".to_string())).await;
        assert_eq!(field.value, "hello world");
        assert_eq!(field.cursor_pos(), 10);
    }

    #[tokio::test]
    async fn multibyte_utf8_navigation() {
        let mut field = TextField::new("a中b".to_string());
        assert_eq!(field.cursor_pos(), 5);

        field.on_event(&Event::Key(key(KeyCode::Left))).await;
        assert_eq!(field.cursor_pos(), 4);

        field.on_event(&Event::Key(key(KeyCode::Left))).await;
        assert_eq!(field.cursor_pos(), 1);

        field.on_event(&Event::Key(key(KeyCode::Left))).await;
        assert_eq!(field.cursor_pos(), 0);

        field.on_event(&Event::Key(key(KeyCode::Right))).await;
        assert_eq!(field.cursor_pos(), 1);

        field.on_event(&Event::Key(key(KeyCode::Right))).await;
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

    #[tokio::test]
    async fn delete_after_cursor_removes_char() {
        let mut field = TextField::new("hello".to_string());
        field.set_cursor_pos(2);
        field.on_event(&Event::Key(key(KeyCode::Delete))).await;
        assert_eq!(field.value, "helo");
        assert_eq!(field.cursor_pos(), 2);
    }

    #[tokio::test]
    async fn delete_after_cursor_at_end_is_noop() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(key(KeyCode::Delete))).await;
        assert_eq!(field.value, "hello");
    }

    #[tokio::test]
    async fn delete_after_cursor_multibyte() {
        let mut field = TextField::new("a中b".to_string());
        field.set_cursor_pos(1);
        field.on_event(&Event::Key(key(KeyCode::Delete))).await;
        assert_eq!(field.value, "ab");
        assert_eq!(field.cursor_pos(), 1);
    }

    #[tokio::test]
    async fn ctrl_a_moves_to_start() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(ctrl(KeyCode::Char('a')))).await;
        assert_eq!(field.cursor_pos(), 0);
    }

    #[tokio::test]
    async fn ctrl_e_moves_to_end() {
        let mut field = TextField::new("hello".to_string());
        field.set_cursor_pos(0);
        field.on_event(&Event::Key(ctrl(KeyCode::Char('e')))).await;
        assert_eq!(field.cursor_pos(), 5);
    }

    #[tokio::test]
    async fn ctrl_w_deletes_word() {
        let mut field = TextField::new("hello world".to_string());
        field.on_event(&Event::Key(ctrl(KeyCode::Char('w')))).await;
        assert_eq!(field.value, "hello ");
        assert_eq!(field.cursor_pos(), 6);
    }

    #[tokio::test]
    async fn ctrl_w_trailing_whitespace() {
        let mut field = TextField::new("hello   ".to_string());
        field.on_event(&Event::Key(ctrl(KeyCode::Char('w')))).await;
        assert_eq!(field.value, "");
        assert_eq!(field.cursor_pos(), 0);
    }

    #[tokio::test]
    async fn ctrl_w_at_start_is_noop() {
        let mut field = TextField::new("hello".to_string());
        field.set_cursor_pos(0);
        field.on_event(&Event::Key(ctrl(KeyCode::Char('w')))).await;
        assert_eq!(field.value, "hello");
        assert_eq!(field.cursor_pos(), 0);
    }

    #[tokio::test]
    async fn ctrl_w_mid_word() {
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(8); // "hello wo|rld"
        field.on_event(&Event::Key(ctrl(KeyCode::Char('w')))).await;
        assert_eq!(field.value, "hello rld");
        assert_eq!(field.cursor_pos(), 6);
    }

    #[tokio::test]
    async fn ctrl_w_does_not_insert_w() {
        let mut field = TextField::new(String::new());
        field.on_event(&Event::Key(ctrl(KeyCode::Char('w')))).await;
        assert_eq!(field.value, "");
    }

    #[tokio::test]
    async fn alt_backspace_deletes_word() {
        let mut field = TextField::new("hello world".to_string());
        field.on_event(&Event::Key(alt(KeyCode::Backspace))).await;
        assert_eq!(field.value, "hello ");
    }

    #[tokio::test]
    async fn ctrl_u_deletes_to_start() {
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(5);
        field.on_event(&Event::Key(ctrl(KeyCode::Char('u')))).await;
        assert_eq!(field.value, " world");
        assert_eq!(field.cursor_pos(), 0);
    }

    #[tokio::test]
    async fn ctrl_u_at_start_is_noop() {
        let mut field = TextField::new("hello".to_string());
        field.set_cursor_pos(0);
        field.on_event(&Event::Key(ctrl(KeyCode::Char('u')))).await;
        assert_eq!(field.value, "hello");
        assert_eq!(field.cursor_pos(), 0);
    }

    #[tokio::test]
    async fn ctrl_k_deletes_to_end() {
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(5);
        field.on_event(&Event::Key(ctrl(KeyCode::Char('k')))).await;
        assert_eq!(field.value, "hello");
        assert_eq!(field.cursor_pos(), 5);
    }

    #[tokio::test]
    async fn ctrl_k_at_end_is_noop() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(ctrl(KeyCode::Char('k')))).await;
        assert_eq!(field.value, "hello");
    }

    #[tokio::test]
    async fn alt_left_moves_word_left() {
        let mut field = TextField::new("hello world".to_string());
        field.on_event(&Event::Key(alt(KeyCode::Left))).await;
        assert_eq!(field.cursor_pos(), 6);
    }

    #[tokio::test]
    async fn alt_left_from_mid_word() {
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(8);
        field.on_event(&Event::Key(alt(KeyCode::Left))).await;
        assert_eq!(field.cursor_pos(), 6);
    }

    #[tokio::test]
    async fn alt_left_at_start_stays() {
        let mut field = TextField::new("hello".to_string());
        field.set_cursor_pos(0);
        field.on_event(&Event::Key(alt(KeyCode::Left))).await;
        assert_eq!(field.cursor_pos(), 0);
    }

    #[tokio::test]
    async fn ctrl_left_moves_word_left() {
        let mut field = TextField::new("hello world".to_string());
        field.on_event(&Event::Key(ctrl(KeyCode::Left))).await;
        assert_eq!(field.cursor_pos(), 6);
    }

    #[tokio::test]
    async fn alt_right_moves_word_right() {
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(0);
        field.on_event(&Event::Key(alt(KeyCode::Right))).await;
        assert_eq!(field.cursor_pos(), 6);
    }

    #[tokio::test]
    async fn alt_right_at_end_stays() {
        let mut field = TextField::new("hello".to_string());
        field.on_event(&Event::Key(alt(KeyCode::Right))).await;
        assert_eq!(field.cursor_pos(), 5);
    }

    #[tokio::test]
    async fn alt_right_multibyte() {
        let mut field = TextField::new("a中 b".to_string());
        field.set_cursor_pos(0);
        field.on_event(&Event::Key(alt(KeyCode::Right))).await;
        assert_eq!(field.cursor_pos(), 5);
    }

    #[tokio::test]
    async fn ctrl_right_moves_word_right() {
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(0);
        field.on_event(&Event::Key(ctrl(KeyCode::Right))).await;
        assert_eq!(field.cursor_pos(), 6);
    }

    #[test]
    fn move_cursor_up_first_row_goes_home() {
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(3);
        field.move_cursor_up(10);
        assert_eq!(field.cursor_pos(), 0);
    }

    #[test]
    fn move_cursor_down_last_row_goes_end() {
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(0);
        field.move_cursor_down(20);
        assert_eq!(field.cursor_pos(), 11);
    }

    #[test]
    fn move_cursor_up_multi_row() {
        // "hello world" with content_width=5:
        // row 0: "hello" (display 0-4)
        // row 1: " worl" (display 5-9)
        // row 2: "d"     (display 10)
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(8); // display width 8, row 1, col 3
        field.move_cursor_up(5);
        // Target: row 0, col 3 → display width 3 → byte 3 ('l')
        assert_eq!(field.cursor_pos(), 3);
    }

    #[test]
    fn move_cursor_down_multi_row() {
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(3); // display width 3, row 0, col 3
        field.move_cursor_down(5);
        // Target: row 1, col 3 → display width 8 → byte 8 ('r')
        assert_eq!(field.cursor_pos(), 8);
    }

    #[test]
    fn move_cursor_down_clamps_to_total_width() {
        // "hello world" with content_width=5:
        // row 2 has only "d" (display 10), col 0
        // Pressing down from row 1, col 3 (display 8) → row 2, col 3 → display 13 → clamped to 11
        let mut field = TextField::new("hello world".to_string());
        field.set_cursor_pos(8);
        field.move_cursor_down(5);
        assert_eq!(field.cursor_pos(), 11);
    }

    #[test]
    fn move_cursor_up_wide_chars() {
        // '中' is 2 display-width columns
        // "中中中中中" = 10 display cols, content_width=5 → 2 rows
        let mut field = TextField::new("中中中中中".to_string());
        // Cursor at end: byte 15, display 10, row 2, col 0
        field.move_cursor_up(5);
        // byte_offset_for_display_width(5): wide chars can't land exactly on the boundary.
        // Returns byte 9 (display width 6, not 5).
        assert_eq!(field.cursor_pos(), 9);
    }

    #[test]
    fn move_cursor_down_on_empty_string() {
        let mut field = TextField::new(String::new());
        field.move_cursor_down(10);
        assert_eq!(field.cursor_pos(), 0);
    }

    #[test]
    fn move_cursor_up_zero_width_is_noop() {
        let mut field = TextField::new("hello".to_string());
        field.set_cursor_pos(3);
        field.move_cursor_up(0);
        assert_eq!(field.cursor_pos(), 3);
    }
}
