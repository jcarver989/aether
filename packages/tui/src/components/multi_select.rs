use crossterm::event::{KeyCode, KeyEvent};

use super::select_option::SelectOption;
use crate::component::{Component, InteractiveComponent, KeyEventResponse, RenderContext};
use crate::line::Line;
use crate::style::Style;

/// Multi-select from a list of options, rendered as checkboxes with a cursor.
pub struct MultiSelect {
    pub options: Vec<SelectOption>,
    pub selected: Vec<bool>,
    pub cursor: usize,
}

impl MultiSelect {
    pub fn new(options: Vec<SelectOption>, selected: Vec<bool>) -> Self {
        Self {
            cursor: 0,
            options,
            selected,
        }
    }

    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> serde_json::Value {
        let values: Vec<serde_json::Value> = self
            .options
            .iter()
            .zip(self.selected.iter())
            .filter(|&(_, &s)| s)
            .map(|(o, _)| serde_json::Value::String(o.value.clone()))
            .collect();
        serde_json::Value::Array(values)
    }
}

impl Component for MultiSelect {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        if context.focused {
            self.render_options(context)
        } else {
            vec![self.render_inline(context)]
        }
    }
}

impl MultiSelect {
    fn render_inline(&self, context: &RenderContext) -> Line {
        let chosen: Vec<&str> = self
            .options
            .iter()
            .zip(self.selected.iter())
            .filter(|&(_, &s)| s)
            .map(|(o, _)| o.title.as_str())
            .collect();

        if chosen.is_empty() {
            Line::styled("(none)", context.theme.muted())
        } else {
            Line::styled(chosen.join(", "), context.theme.info())
        }
    }

    fn render_options(&self, context: &RenderContext) -> Vec<Line> {
        self.options
            .iter()
            .enumerate()
            .map(|(j, opt)| {
                let marker = if self.selected[j] { "[x] " } else { "[ ] " };
                let is_cursor = j == self.cursor;
                let style = if is_cursor {
                    Style::fg(context.theme.primary()).bold()
                } else if self.selected[j] {
                    Style::fg(context.theme.primary())
                } else {
                    Style::default()
                };
                Line::with_style(format!("{marker}{}", opt.title), style)
            })
            .collect()
    }
}

impl InteractiveComponent for MultiSelect {
    type Action = ();

    fn on_key_event(&mut self, key_event: KeyEvent) -> KeyEventResponse<()> {
        if self.options.is_empty() {
            return KeyEventResponse::ignored();
        }

        match key_event.code {
            KeyCode::Char(' ') => {
                self.selected[self.cursor] = !self.selected[self.cursor];
                KeyEventResponse::consumed()
            }
            KeyCode::Up | KeyCode::Left => {
                self.cursor = (self.cursor + self.options.len() - 1) % self.options.len();
                KeyEventResponse::consumed()
            }
            KeyCode::Down | KeyCode::Right => {
                self.cursor = (self.cursor + 1) % self.options.len();
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

    fn sample() -> MultiSelect {
        MultiSelect::new(
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
            ],
            vec![false, false, false],
        )
    }

    #[test]
    fn space_toggles_at_cursor() {
        let mut ms = sample();
        ms.on_key_event(key(KeyCode::Char(' ')));
        assert!(ms.selected[0]);
        ms.on_key_event(key(KeyCode::Char(' ')));
        assert!(!ms.selected[0]);
    }

    #[test]
    fn cursor_moves_with_arrows() {
        let mut ms = sample();
        ms.on_key_event(key(KeyCode::Down));
        assert_eq!(ms.cursor, 1);
        ms.on_key_event(key(KeyCode::Char(' ')));
        assert!(ms.selected[1]);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn to_json_returns_selected_values() {
        let mut ms = sample();
        ms.selected[0] = true;
        ms.selected[2] = true;
        assert_eq!(ms.to_json(), serde_json::json!(["a", "c"]));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn to_json_empty_selection() {
        let ms = sample();
        assert_eq!(ms.to_json(), serde_json::json!([]));
    }

    #[test]
    fn cursor_wraps() {
        let mut ms = sample();
        ms.on_key_event(key(KeyCode::Up));
        assert_eq!(ms.cursor, 2); // wraps to end
    }
}
