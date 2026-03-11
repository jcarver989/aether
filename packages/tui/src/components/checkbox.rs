use crossterm::event::KeyCode;

use crate::components::{Outcome, ViewContext, Widget, WidgetEvent};
use crate::line::Line;

/// Boolean toggle rendered as `[x]` / `[ ]`.
pub struct Checkbox {
    pub checked: bool,
}

impl Checkbox {
    pub fn new(checked: bool) -> Self {
        Self { checked }
    }

    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Bool(self.checked)
    }
}

impl Widget for Checkbox {
    type Message = ();

    fn on_event(&mut self, event: &WidgetEvent) -> Outcome<Self::Message> {
        let WidgetEvent::Key(key) = event else {
            return Outcome::ignored();
        };
        match key.code {
            KeyCode::Char(' ') => {
                self.checked = !self.checked;
                Outcome::consumed()
            }
            _ => Outcome::ignored(),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        let display = if self.checked { "[x]" } else { "[ ]" };
        let style = if context.focused {
            context.theme.primary()
        } else {
            context.theme.text_primary()
        };
        vec![Line::styled(display, style)]
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
    fn space_toggles() {
        let mut cb = Checkbox::new(false);
        cb.on_event(&WidgetEvent::Key(key(KeyCode::Char(' '))));
        assert!(cb.checked);
        cb.on_event(&WidgetEvent::Key(key(KeyCode::Char(' '))));
        assert!(!cb.checked);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn to_json_returns_bool() {
        assert_eq!(Checkbox::new(true).to_json(), serde_json::json!(true));
        assert_eq!(Checkbox::new(false).to_json(), serde_json::json!(false));
    }

    #[test]
    fn other_keys_are_ignored() {
        let mut cb = Checkbox::new(false);
        let outcome = cb.on_event(&WidgetEvent::Key(key(KeyCode::Char('a'))));
        assert!(!outcome.is_handled());
        assert!(!cb.checked);
    }
}
