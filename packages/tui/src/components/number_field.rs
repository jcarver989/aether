use crossterm::event::{KeyCode, KeyEvent};

use crate::component::{Component, InteractiveComponent, KeyEventResponse, RenderContext};
use crate::line::Line;

/// Numeric input field supporting integers or floats.
pub struct NumberField {
    pub value: String,
    pub integer_only: bool,
}

impl NumberField {
    pub fn new(value: String, integer_only: bool) -> Self {
        Self {
            value,
            integer_only,
        }
    }

    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> serde_json::Value {
        if self.integer_only {
            self.value
                .parse::<i64>()
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null)
        } else {
            self.value
                .parse::<f64>()
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null)
        }
    }
}

impl Component for NumberField {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut line = Line::new(&self.value);
        if context.focused {
            line.push_styled("▏", context.theme.primary());
        }
        vec![line]
    }
}

impl InteractiveComponent for NumberField {
    type Action = ();

    fn on_key_event(&mut self, key_event: KeyEvent) -> KeyEventResponse<()> {
        match key_event.code {
            KeyCode::Char(c) => {
                let accept = c.is_ascii_digit()
                    || (c == '-' && self.value.is_empty())
                    || (c == '.' && !self.integer_only && !self.value.contains('.'));
                if accept {
                    self.value.push(c);
                    KeyEventResponse::consumed_and_render()
                } else {
                    KeyEventResponse::consumed()
                }
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
    fn integer_accepts_digits_and_leading_minus() {
        let mut field = NumberField::new(String::new(), true);
        field.on_key_event(key(KeyCode::Char('-')));
        field.on_key_event(key(KeyCode::Char('4')));
        field.on_key_event(key(KeyCode::Char('2')));
        assert_eq!(field.value, "-42");
    }

    #[test]
    fn integer_rejects_dot() {
        let mut field = NumberField::new("1".to_string(), true);
        field.on_key_event(key(KeyCode::Char('.')));
        assert_eq!(field.value, "1");
    }

    #[test]
    fn float_accepts_single_dot() {
        let mut field = NumberField::new(String::new(), false);
        field.on_key_event(key(KeyCode::Char('3')));
        field.on_key_event(key(KeyCode::Char('.')));
        field.on_key_event(key(KeyCode::Char('5')));
        assert_eq!(field.value, "3.5");
    }

    #[test]
    fn float_rejects_second_dot() {
        let mut field = NumberField::new("1.2".to_string(), false);
        field.on_key_event(key(KeyCode::Char('.')));
        assert_eq!(field.value, "1.2");
    }

    #[test]
    fn minus_rejected_when_not_first() {
        let mut field = NumberField::new("5".to_string(), true);
        field.on_key_event(key(KeyCode::Char('-')));
        assert_eq!(field.value, "5");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn to_json_integer() {
        let field = NumberField::new("42".to_string(), true);
        assert_eq!(field.to_json(), serde_json::json!(42));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn to_json_float() {
        let field = NumberField::new("3.14".to_string(), false);
        #[allow(clippy::approx_constant)]
        let expected = serde_json::json!(3.14);
        assert_eq!(field.to_json(), expected);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn to_json_empty_returns_null() {
        let field = NumberField::new(String::new(), true);
        assert_eq!(field.to_json(), serde_json::Value::Null);
    }

    #[test]
    fn backspace_removes_last() {
        let mut field = NumberField::new("12".to_string(), true);
        field.on_key_event(key(KeyCode::Backspace));
        assert_eq!(field.value, "1");
    }
}
