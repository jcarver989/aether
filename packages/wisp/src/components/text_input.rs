use crate::keybindings::Keybindings;
use crate::tui::{Component, Event, KeyEvent, Line, TextField, ViewContext};
use std::path::PathBuf;

pub struct TextInput {
    field: TextField,
    mentions: Vec<SelectedFileMention>,
    keybindings: Keybindings,
}

pub enum TextInputMessage {
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
        Self::new(Keybindings::default())
    }
}

impl TextInput {
    pub fn new(keybindings: Keybindings) -> Self {
        Self {
            field: TextField::new(String::new()),
            mentions: Vec::new(),
            keybindings,
        }
    }

    pub fn buffer(&self) -> &str {
        &self.field.value
    }

    /// Returns the visual cursor index, accounting for an active file picker
    /// whose query extends beyond the `@` trigger character.
    pub fn cursor_index(&self, picker_query_len: Option<usize>) -> usize {
        if let Some(query_len) = picker_query_len {
            let at_pos = self
                .active_mention_start()
                .unwrap_or(self.field.value.len());
            at_pos + 1 + query_len
        } else {
            self.field.cursor_pos()
        }
    }

    #[cfg(test)]
    pub fn mentions(&self) -> &[SelectedFileMention] {
        &self.mentions
    }

    pub fn take_mentions(&mut self) -> Vec<SelectedFileMention> {
        std::mem::take(&mut self.mentions)
    }

    pub fn set_input(&mut self, s: String) {
        self.field.set_value(s);
    }

    #[cfg(test)]
    pub fn set_cursor_pos(&mut self, pos: usize) {
        self.field.set_cursor_pos(pos);
    }

    pub fn clear(&mut self) {
        self.field.clear();
    }

    pub fn insert_char_at_cursor(&mut self, c: char) {
        self.field.insert_at_cursor(c);
    }

    pub fn delete_char_before_cursor(&mut self) -> bool {
        self.field.delete_before_cursor()
    }

    pub fn insert_paste(&mut self, text: &str) {
        for c in text.chars() {
            if !c.is_control() {
                self.field.insert_at_cursor(c);
            }
        }
    }

    pub fn apply_file_selection(&mut self, path: PathBuf, display_name: String) {
        let mention = format!("@{display_name}");
        self.mentions.push(SelectedFileMention {
            mention: mention.clone(),
            path,
            display_name,
        });

        if let Some(at_pos) = self.active_mention_start() {
            let mut s = self.field.value[..at_pos].to_string();
            s.push_str(&mention);
            s.push(' ');
            self.set_input(s);
        }
    }

    fn active_mention_start(&self) -> Option<usize> {
        mention_start(&self.field.value)
    }
}

impl Component for TextInput {
    type Message = TextInputMessage;

    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        match event {
            Event::Paste(text) => {
                self.insert_paste(text);
                Some(vec![])
            }
            Event::Key(key_event) => self.handle_key(key_event),
            _ => None,
        }
    }

    fn render(&self, _context: &ViewContext) -> Vec<Line> {
        vec![Line::new(self.field.value.clone())]
    }
}

impl TextInput {
    fn handle_key(&mut self, key_event: &KeyEvent) -> Option<Vec<TextInputMessage>> {
        if self.keybindings.submit.matches(*key_event) {
            return Some(vec![TextInputMessage::Submit]);
        }

        if self.keybindings.open_command_picker.matches(*key_event) && self.field.value.is_empty() {
            if let Some(c) = self.keybindings.open_command_picker.char() {
                self.field.insert_at_cursor(c);
            }
            return Some(vec![TextInputMessage::OpenCommandPicker]);
        }

        if self.keybindings.open_file_picker.matches(*key_event) {
            if let Some(c) = self.keybindings.open_file_picker.char() {
                self.field.insert_at_cursor(c);
            }
            return Some(vec![TextInputMessage::OpenFilePicker]);
        }

        // Delegate cursor navigation, char input, and backspace to TextField
        self.field.on_event(&Event::Key(*key_event)).map(|_| vec![])
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
    use crate::tui::KeyCode;
    use crate::tui::KeyModifiers;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn left_arrow_moves_cursor_back_one_char() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());

        input.on_event(&key(KeyCode::Left));

        assert_eq!(input.cursor_index(None), 4);
    }

    #[test]
    fn right_arrow_moves_cursor_forward_one_char() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());
        input.set_cursor_pos(2);

        input.on_event(&key(KeyCode::Right));

        assert_eq!(input.cursor_index(None), 3);
    }

    #[test]
    fn left_at_start_stays_at_zero() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());
        input.set_cursor_pos(0);

        input.on_event(&key(KeyCode::Left));

        assert_eq!(input.cursor_index(None), 0);
    }

    #[test]
    fn right_at_end_stays_at_end() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());

        input.on_event(&key(KeyCode::Right));

        assert_eq!(input.cursor_index(None), 5);
    }

    #[test]
    fn home_moves_to_start() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());
        input.set_cursor_pos(3);

        input.on_event(&key(KeyCode::Home));

        assert_eq!(input.cursor_index(None), 0);
    }

    #[test]
    fn end_moves_to_end() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());
        input.set_cursor_pos(1);

        input.on_event(&key(KeyCode::End));

        assert_eq!(input.cursor_index(None), 5);
    }

    #[test]
    fn typing_inserts_at_cursor_position() {
        let mut input = TextInput::default();
        input.set_input("hllo".to_string());
        input.set_cursor_pos(1);

        input.on_event(&key(KeyCode::Char('e')));

        assert_eq!(input.buffer(), "hello");
        assert_eq!(input.cursor_index(None), 2);
    }

    #[test]
    fn backspace_at_cursor_middle_deletes_correct_char() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());
        input.set_cursor_pos(3);

        input.on_event(&key(KeyCode::Backspace));

        assert_eq!(input.buffer(), "helo");
        assert_eq!(input.cursor_index(None), 2);
    }

    #[test]
    fn backspace_at_start_does_nothing() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());
        input.set_cursor_pos(0);

        let outcome = input.on_event(&key(KeyCode::Backspace));

        assert!(outcome.is_some());
        assert_eq!(input.buffer(), "hello");
        assert_eq!(input.cursor_index(None), 0);
    }

    #[test]
    fn multibyte_utf8_cursor_navigation() {
        let mut input = TextInput::default();
        // "a中b" — 'a' is 1 byte, '中' is 3 bytes, 'b' is 1 byte = 5 bytes total
        input.set_input("a中b".to_string());

        input.on_event(&key(KeyCode::Left));
        assert_eq!(input.cursor_index(None), 4); // before 'b'

        input.on_event(&key(KeyCode::Left));
        assert_eq!(input.cursor_index(None), 1); // before '中'

        input.on_event(&key(KeyCode::Left));
        assert_eq!(input.cursor_index(None), 0); // before 'a'

        input.on_event(&key(KeyCode::Right));
        assert_eq!(input.cursor_index(None), 1); // after 'a'

        input.on_event(&key(KeyCode::Right));
        assert_eq!(input.cursor_index(None), 4); // after '中'
    }

    #[test]
    fn paste_inserts_at_cursor_position() {
        let mut input = TextInput::default();
        input.set_input("hd".to_string());
        input.set_cursor_pos(1);

        input.insert_paste("ello worl");

        assert_eq!(input.buffer(), "hello world");
        assert_eq!(input.cursor_index(None), 10);
    }

    #[test]
    fn slash_on_empty_returns_open_command_picker() {
        let mut input = TextInput::default();

        let outcome = input.on_event(&key(KeyCode::Char('/')));

        assert!(outcome.is_some());
        assert!(matches!(
            outcome.as_deref(),
            Some([TextInputMessage::OpenCommandPicker])
        ));
        assert_eq!(input.buffer(), "/");
    }

    #[test]
    fn at_sign_returns_open_file_picker() {
        let mut input = TextInput::default();

        let outcome = input.on_event(&key(KeyCode::Char('@')));

        assert!(outcome.is_some());
        assert!(matches!(
            outcome.as_deref(),
            Some([TextInputMessage::OpenFilePicker])
        ));
        assert_eq!(input.buffer(), "@");
    }

    #[test]
    fn enter_returns_submit() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());

        let outcome = input.on_event(&key(KeyCode::Enter));

        assert!(outcome.is_some());
        assert!(matches!(
            outcome.as_deref(),
            Some([TextInputMessage::Submit])
        ));
    }

    #[test]
    fn file_selection_updates_mentions_and_buffer() {
        let mut input = TextInput::default();
        input.set_input("@fo".to_string());

        input.apply_file_selection(PathBuf::from("foo.rs"), "foo.rs".to_string());

        assert_eq!(input.buffer(), "@foo.rs ");
        assert_eq!(input.mentions().len(), 1);
        assert_eq!(input.mentions()[0].mention, "@foo.rs");
    }

    #[test]
    fn cursor_index_without_picker() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());
        input.set_cursor_pos(3);

        assert_eq!(input.cursor_index(None), 3);
    }

    #[test]
    fn cursor_index_with_picker_query() {
        let mut input = TextInput::default();
        input.set_input("@fo".to_string());

        // Picker has 2-char query ("fo"), @ is at position 0
        assert_eq!(input.cursor_index(Some(2)), 3); // 0 + 1 + 2
    }

    #[test]
    fn clear_resets_buffer_and_cursor() {
        let mut input = TextInput::default();
        input.set_input("hello".to_string());

        input.clear();

        assert_eq!(input.buffer(), "");
        assert_eq!(input.cursor_index(None), 0);
    }
}
