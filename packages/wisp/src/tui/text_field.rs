use crossterm::event::{KeyCode, KeyEvent};

use super::component::{Component, HandlesInput, InputOutcome, RenderContext};
use super::screen::Line;

/// Single-line text input with cursor indicator.
pub struct TextField {
    pub value: String,
}

impl TextField {
    pub fn new(value: String) -> Self {
        Self { value }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::String(self.value.clone())
    }
}

impl Component for TextField {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let mut line = Line::new(&self.value);
        line.push_styled("▏", context.theme.primary);
        vec![line]
    }
}

impl HandlesInput for TextField {
    type Action = ();

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<()> {
        match key_event.code {
            KeyCode::Char(c) => {
                self.value.push(c);
                InputOutcome::consumed_and_render()
            }
            KeyCode::Backspace => {
                self.value.pop();
                InputOutcome::consumed_and_render()
            }
            _ => InputOutcome::ignored(),
        }
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
        field.handle_key(key(KeyCode::Char('h')));
        field.handle_key(key(KeyCode::Char('i')));
        assert_eq!(field.value, "hi");
    }

    #[test]
    fn backspace_removes_last_character() {
        let mut field = TextField::new("abc".to_string());
        field.handle_key(key(KeyCode::Backspace));
        assert_eq!(field.value, "ab");
    }

    #[test]
    fn backspace_on_empty_is_no_op() {
        let mut field = TextField::new(String::new());
        field.handle_key(key(KeyCode::Backspace));
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
        let outcome = field.handle_key(key(KeyCode::Up));
        assert!(!outcome.consumed);
    }
}
