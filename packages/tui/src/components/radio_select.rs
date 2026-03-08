use crossterm::event::{KeyCode, KeyEvent};

use crate::component::{Component, HandlesInput, InputOutcome, RenderContext};
use crate::line::Line;
use crate::style::Style;
use super::select_option::SelectOption;

/// Single-select from a list of options, rendered as radio buttons.
pub struct RadioSelect {
    pub options: Vec<SelectOption>,
    pub selected: usize,
}

impl RadioSelect {
    pub fn new(options: Vec<SelectOption>, selected: usize) -> Self {
        Self { options, selected }
    }

    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> serde_json::Value {
        self.options
            .get(self.selected)
            .map_or(serde_json::Value::Null, |o| {
                serde_json::Value::String(o.value.clone())
            })
    }
}

impl Component for RadioSelect {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        if context.focused {
            self.render_options(context)
        } else {
            vec![self.render_inline(context)]
        }
    }
}

impl RadioSelect {
    fn render_inline(&self, context: &RenderContext) -> Line {
        if let Some(opt) = self.options.get(self.selected) {
            Line::styled(&opt.title, context.theme.info())
        } else {
            Line::default()
        }
    }

    fn render_options(&self, context: &RenderContext) -> Vec<Line> {
        self.options
            .iter()
            .enumerate()
            .map(|(j, opt)| {
                let marker = if j == self.selected { "● " } else { "○ " };
                let style = if j == self.selected {
                    Style::fg(context.theme.primary())
                } else {
                    Style::default()
                };
                Line::with_style(format!("{marker}{}", opt.title), style)
            })
            .collect()
    }
}

impl HandlesInput for RadioSelect {
    type Action = ();

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<()> {
        if self.options.is_empty() {
            return InputOutcome::ignored();
        }

        match key_event.code {
            KeyCode::Left | KeyCode::Up => {
                self.selected = (self.selected + self.options.len() - 1) % self.options.len();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Right | KeyCode::Down => {
                self.selected = (self.selected + 1) % self.options.len();
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

    fn sample_options() -> Vec<SelectOption> {
        vec![
            SelectOption {
                value: "a".into(),
                title: "Alpha".into(),
            },
            SelectOption {
                value: "b".into(),
                title: "Beta".into(),
            },
            SelectOption {
                value: "c".into(),
                title: "Gamma".into(),
            },
        ]
    }

    #[test]
    fn right_cycles_forward() {
        let mut rs = RadioSelect::new(sample_options(), 0);
        rs.handle_key(key(KeyCode::Right));
        assert_eq!(rs.selected, 1);
        rs.handle_key(key(KeyCode::Right));
        assert_eq!(rs.selected, 2);
        rs.handle_key(key(KeyCode::Right));
        assert_eq!(rs.selected, 0); // wraps
    }

    #[test]
    fn left_cycles_backward() {
        let mut rs = RadioSelect::new(sample_options(), 0);
        rs.handle_key(key(KeyCode::Left));
        assert_eq!(rs.selected, 2); // wraps to end
    }

    #[cfg(feature = "serde")]
    #[test]
    fn to_json_returns_selected_value() {
        let rs = RadioSelect::new(sample_options(), 1);
        assert_eq!(rs.to_json(), serde_json::json!("b"));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn to_json_empty_options_returns_null() {
        let rs = RadioSelect::new(vec![], 0);
        assert_eq!(rs.to_json(), serde_json::Value::Null);
    }

    #[test]
    fn empty_options_ignores_keys() {
        let mut rs = RadioSelect::new(vec![], 0);
        let outcome = rs.handle_key(key(KeyCode::Right));
        assert!(!outcome.consumed);
    }
}
