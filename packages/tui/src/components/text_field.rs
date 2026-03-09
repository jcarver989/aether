use crossterm::event::KeyCode;

use crate::component::{Component, InteractiveComponent, MessageResult, RenderContext, UiEvent};
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
    type Message = ();

    fn on_event(&mut self, event: UiEvent) -> MessageResult<Self::Message> {
        match event {
            UiEvent::Key(key_event) => match key_event.code {
                KeyCode::Char(c) => {
                    self.value.push(c);
                    MessageResult::consumed()
                }
                KeyCode::Backspace => {
                    self.value.pop();
                    MessageResult::consumed()
                }
                _ => MessageResult::ignored(),
            },
            UiEvent::Paste(text) => {
                self.value.push_str(&text);
                MessageResult::consumed()
            }
            UiEvent::Tick(_) => MessageResult::ignored(),
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
        field.on_event(UiEvent::Key(key(KeyCode::Char('h'))));
        field.on_event(UiEvent::Key(key(KeyCode::Char('i'))));
        assert_eq!(field.value, "hi");
    }

    #[test]
    fn backspace_removes_last_character() {
        let mut field = TextField::new("abc".to_string());
        field.on_event(UiEvent::Key(key(KeyCode::Backspace)));
        assert_eq!(field.value, "ab");
    }

    #[test]
    fn backspace_on_empty_is_no_op() {
        let mut field = TextField::new(String::new());
        field.on_event(UiEvent::Key(key(KeyCode::Backspace)));
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
        let outcome = field.on_event(UiEvent::Key(key(KeyCode::Up)));
        assert!(!outcome.handled);
    }

    #[test]
    fn paste_appends_text() {
        let mut field = TextField::new(String::new());
        let outcome = field.on_event(UiEvent::Paste("hello".to_string()));
        assert!(outcome.handled);
        assert_eq!(field.value, "hello");
    }
}
