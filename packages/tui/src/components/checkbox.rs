use crossterm::event::{KeyCode, KeyEvent};

use crate::component::{Component, InteractiveComponent, KeyEventResponse, RenderContext};
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

impl Component for Checkbox {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let display = if self.checked { "[x]" } else { "[ ]" };
        let style = if context.focused {
            context.theme.primary()
        } else {
            context.theme.text_primary()
        };
        vec![Line::styled(display, style)]
    }
}

impl InteractiveComponent for Checkbox {
    type Action = ();

    fn on_key_event(&mut self, key_event: KeyEvent) -> KeyEventResponse<()> {
        match key_event.code {
            KeyCode::Char(' ') => {
                self.checked = !self.checked;
                KeyEventResponse::consumed()
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
    fn space_toggles() {
        let mut cb = Checkbox::new(false);
        cb.on_key_event(key(KeyCode::Char(' ')));
        assert!(cb.checked);
        cb.on_key_event(key(KeyCode::Char(' ')));
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
        let outcome = cb.on_key_event(key(KeyCode::Char('a')));
        assert!(!outcome.consumed);
        assert!(!cb.checked);
    }
}
