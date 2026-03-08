use crossterm::event::{KeyCode, KeyEvent};

use crate::component::{Component, InteractiveComponent, KeyEventResponse, RenderContext};
use crate::line::Line;

/// Single-line text input with cursor indicator.
pub struct TextField {
    pub value: String,
}

impl TextField {
    pub fn new(value: String) -> Self {
        Self { value }
    }

    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::String(self.value.clone())
    }
}

impl Component for TextField {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut line = Line::new(&self.value);
        if context.focused {
            line.push_styled("▏", context.theme.primary());
        }
        vec![line]
    }
}

impl InteractiveComponent for TextField {
    type Action = ();

    fn on_key_event(&mut self, key_event: KeyEvent) -> KeyEventResponse<()> {
        match key_event.code {
            KeyCode::Char(c) => {
                self.value.push(c);
                KeyEventResponse::consumed_and_render()
            }
            KeyCode::Backspace => {
                self.value.pop();
                KeyEventResponse::consumed_and_render()
            }
            _ => KeyEventResponse::ignored(),
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
        field.on_key_event(key(KeyCode::Char('h')));
        field.on_key_event(key(KeyCode::Char('i')));
        assert_eq!(field.value, "hi");
    }

    #[test]
    fn backspace_removes_last_character() {
        let mut field = TextField::new("abc".to_string());
        field.on_key_event(key(KeyCode::Backspace));
        assert_eq!(field.value, "ab");
    }

    #[test]
    fn backspace_on_empty_is_no_op() {
        let mut field = TextField::new(String::new());
        field.on_key_event(key(KeyCode::Backspace));
        assert_eq!(field.value, "");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn to_json_returns_string_value() {
        let field = TextField::new("hello".to_string());
        assert_eq!(field.to_json(), serde_json::json!("hello"));
    }

    #[test]
    fn unhandled_keys_are_ignored() {
        let mut field = TextField::new(String::new());
        let outcome = field.on_key_event(key(KeyCode::Up));
        assert!(!outcome.consumed);
    }
}
