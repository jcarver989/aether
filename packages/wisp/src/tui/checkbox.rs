use crossterm::event::{KeyCode, KeyEvent};

use super::component::{Component, HandlesInput, InputOutcome, RenderContext};
use super::screen::Line;

/// Boolean toggle rendered as `[x]` / `[ ]`.
pub struct Checkbox {
    pub checked: bool,
}

impl Checkbox {
    pub fn new(checked: bool) -> Self {
        Self { checked }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Bool(self.checked)
    }
}

impl Component for Checkbox {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let display = if self.checked { "[x]" } else { "[ ]" };
        let style = if context.focused {
            context.theme.primary()
        } else {
            context.theme.text_primary()
        };
        vec![Line::styled(display, style)]
    }
}

impl HandlesInput for Checkbox {
    type Action = ();

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<()> {
        match key_event.code {
            KeyCode::Char(' ') => {
                self.checked = !self.checked;
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
    fn space_toggles() {
        let mut cb = Checkbox::new(false);
        cb.handle_key(key(KeyCode::Char(' ')));
        assert!(cb.checked);
        cb.handle_key(key(KeyCode::Char(' ')));
        assert!(!cb.checked);
    }

    #[test]
    fn to_json_returns_bool() {
        assert_eq!(Checkbox::new(true).to_json(), serde_json::json!(true));
        assert_eq!(Checkbox::new(false).to_json(), serde_json::json!(false));
    }

    #[test]
    fn other_keys_are_ignored() {
        let mut cb = Checkbox::new(false);
        let outcome = cb.handle_key(key(KeyCode::Char('a')));
        assert!(!outcome.consumed);
        assert!(!cb.checked);
    }
}
