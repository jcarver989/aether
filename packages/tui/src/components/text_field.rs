use crossterm::event::KeyCode;

use crate::components::{Outcome, ViewContext, Widget, WidgetEvent};
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

impl Widget for TextField {
    type Message = ();

    fn on_event(&mut self, event: &WidgetEvent) -> Outcome<Self::Message> {
        match event {
            WidgetEvent::Key(key) => match key.code {
                KeyCode::Char(c) => {
                    self.value.push(c);
                    Outcome::consumed()
                }
                KeyCode::Backspace => {
                    self.value.pop();
                    Outcome::consumed()
                }
                _ => Outcome::ignored(),
            },
            WidgetEvent::Paste(text) => {
                self.value.push_str(text);
                Outcome::consumed()
            }
            _ => Outcome::ignored(),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        let mut line = Line::new(&self.value);
        if context.focused {
            line.push_styled("▏", context.theme.primary());
        }
        vec![line]
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
        field.on_event(&WidgetEvent::Key(key(KeyCode::Char('h'))));
        field.on_event(&WidgetEvent::Key(key(KeyCode::Char('i'))));
        assert_eq!(field.value, "hi");
    }

    #[test]
    fn backspace_removes_last_character() {
        let mut field = TextField::new("abc".to_string());
        field.on_event(&WidgetEvent::Key(key(KeyCode::Backspace)));
        assert_eq!(field.value, "ab");
    }

    #[test]
    fn backspace_on_empty_is_no_op() {
        let mut field = TextField::new(String::new());
        field.on_event(&WidgetEvent::Key(key(KeyCode::Backspace)));
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
        let outcome = field.on_event(&WidgetEvent::Key(key(KeyCode::Up)));
        assert!(!outcome.is_handled());
    }

    #[test]
    fn paste_appends_text() {
        let mut field = TextField::new(String::new());
        let outcome = field.on_event(&WidgetEvent::Paste("hello".to_string()));
        assert!(outcome.is_handled());
        assert_eq!(field.value, "hello");
    }
}
