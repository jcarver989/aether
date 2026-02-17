use crate::components::config_menu::{ConfigChange, ConfigMenuEntry, ConfigMenuValue};
use crate::tui::{Combobox, Searchable};
use crate::tui::{Component, HandlesInput, InputOutcome, Line, RenderContext};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::Stylize;

impl Searchable for ConfigMenuValue {
    fn search_text(&self) -> String {
        format!("{} {}", self.name, self.value)
    }
}

pub struct ConfigPicker {
    pub config_id: String,
    pub title: String,
    pub combobox: Combobox<ConfigMenuValue>,
    current_value: String,
}

pub enum ConfigPickerAction {
    Close,
    ApplySelection(Option<ConfigChange>),
}

impl ConfigPicker {
    pub fn from_entry(entry: &ConfigMenuEntry) -> Option<Self> {
        let current_value = entry.values.get(entry.current_value_index)?.value.clone();
        let mut picker = Self {
            config_id: entry.config_id.clone(),
            title: entry.title.clone(),
            current_value,
            combobox: Combobox::new(entry.values.clone()),
        };
        picker.combobox.selected_index = picker
            .combobox
            .matches
            .iter()
            .position(|m| m.value == picker.current_value)
            .unwrap_or(0);
        picker.ensure_selectable();
        Some(picker)
    }

    pub fn update_query(&mut self, query: String) {
        self.combobox.update_query(query);
        self.ensure_selectable();
    }

    pub fn push_query_char(&mut self, c: char) {
        self.combobox.push_query_char(c);
        self.ensure_selectable();
    }

    pub fn pop_query_char(&mut self) {
        self.combobox.pop_query_char();
        self.ensure_selectable();
    }

    pub fn move_selection_up(&mut self) {
        let len = self.combobox.matches.len();
        let mut idx = self.combobox.selected_index;
        for _ in 0..len {
            idx = (idx + len - 1) % len;
            if !self.combobox.matches[idx].is_disabled {
                self.combobox.selected_index = idx;
                return;
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        let len = self.combobox.matches.len();
        let mut idx = self.combobox.selected_index;
        for _ in 0..len {
            idx = (idx + 1) % len;
            if !self.combobox.matches[idx].is_disabled {
                self.combobox.selected_index = idx;
                return;
            }
        }
    }

    pub fn confirm_selection(&self) -> Option<ConfigChange> {
        let selected = self.combobox.selected()?;
        if selected.is_disabled || selected.value == self.current_value {
            return None;
        }

        Some(ConfigChange {
            config_id: self.config_id.clone(),
            new_value: selected.value.clone(),
        })
    }

    fn first_enabled_index(&self) -> Option<usize> {
        self.combobox.matches.iter().position(|m| !m.is_disabled)
    }

    fn ensure_selectable(&mut self) {
        if self.combobox.matches.is_empty() {
            self.combobox.selected_index = 0;
            return;
        }
        if self.combobox.selected_index >= self.combobox.matches.len()
            || self.combobox.matches[self.combobox.selected_index].is_disabled
        {
            self.combobox.selected_index = self.first_enabled_index().unwrap_or(0);
        }
    }
}

impl Component for ConfigPicker {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        let header = format!("  {} search: {}", self.title, self.combobox.query);
        lines.push(Line::new(header.with(context.theme.muted).to_string()));

        if self.combobox.matches.is_empty() {
            lines.push(Line::new("  (no matches found)".to_string()));
            return lines;
        }

        for (i, option) in self.combobox.matches.iter().enumerate() {
            let prefix = if i == self.combobox.selected_index {
                "▶ "
            } else {
                "  "
            };
            let label = if option.name == option.value {
                option.name.clone()
            } else {
                format!("{} ({})", option.name, option.value)
            };

            let label = if option.is_disabled {
                if let Some(reason) = option.description.as_deref() {
                    format!("{label} - {reason}")
                } else {
                    label
                }
            } else {
                label
            };

            let line_text = format!("{}{}", prefix, label);
            let line = if option.is_disabled {
                Line::new(line_text.with(context.theme.muted).to_string())
            } else if i == self.combobox.selected_index {
                Line::new(line_text.with(context.theme.primary).to_string())
            } else {
                Line::new(line_text)
            };
            lines.push(line);
        }

        lines
    }
}

impl HandlesInput for ConfigPicker {
    type Action = ConfigPickerAction;

    fn handle_key(
        &mut self,
        key_event: KeyEvent,
        _input: &mut String,
    ) -> InputOutcome<Self::Action> {
        match key_event.code {
            KeyCode::Esc => InputOutcome::action_and_render(ConfigPickerAction::Close),
            KeyCode::Up => {
                self.move_selection_up();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Char('p') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_selection_up();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Down => {
                self.move_selection_down();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Char('n') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_selection_down();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Enter => {
                let change = self.confirm_selection();
                InputOutcome::action_and_render(ConfigPickerAction::ApplySelection(change))
            }
            KeyCode::Char(c) => {
                if c.is_control() {
                    return InputOutcome::consumed();
                }
                self.push_query_char(c);
                InputOutcome::consumed_and_render()
            }
            KeyCode::Backspace => {
                if self.combobox.query.is_empty() {
                    InputOutcome::consumed()
                } else {
                    self.pop_query_char();
                    InputOutcome::consumed_and_render()
                }
            }
            _ => InputOutcome::consumed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn entry() -> ConfigMenuEntry {
        ConfigMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            values: vec![
                ConfigMenuValue {
                    value: "openrouter:openai/gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    description: None,
                    is_disabled: false,
                },
                ConfigMenuValue {
                    value: "openrouter:anthropic/claude-3.5-sonnet".to_string(),
                    name: "Claude Sonnet".to_string(),
                    description: None,
                    is_disabled: false,
                },
                ConfigMenuValue {
                    value: "openrouter:google/gemini-2.5-pro".to_string(),
                    name: "Gemini 2.5 Pro".to_string(),
                    description: None,
                    is_disabled: false,
                },
            ],
            current_value_index: 0,
        }
    }

    #[test]
    fn initializes_with_current_value_selected() {
        let picker = ConfigPicker::from_entry(&entry()).expect("picker");
        assert_eq!(picker.combobox.selected_index, 0);
        assert_eq!(
            picker.combobox.matches[picker.combobox.selected_index].name,
            "GPT-4o"
        );
    }

    #[test]
    fn query_filters_by_name_or_value() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");
        picker.update_query("gemini".to_string());
        assert_eq!(picker.combobox.matches.len(), 1);
        assert_eq!(picker.combobox.matches[0].name, "Gemini 2.5 Pro");

        picker.update_query("anthropic/claude".to_string());
        assert_eq!(picker.combobox.matches.len(), 1);
        assert_eq!(picker.combobox.matches[0].name, "Claude Sonnet");
    }

    #[test]
    fn confirm_selection_omits_unchanged_value() {
        let picker = ConfigPicker::from_entry(&entry()).expect("picker");
        assert!(picker.confirm_selection().is_none());
    }

    #[test]
    fn confirm_selection_returns_change_for_new_value() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");
        picker.move_selection_down();
        let change = picker.confirm_selection().expect("config change");
        assert_eq!(change.config_id, "model");
        assert_eq!(
            change.new_value,
            "openrouter:anthropic/claude-3.5-sonnet".to_string()
        );
    }

    #[test]
    fn disabled_option_cannot_be_confirmed() {
        let mut entry = entry();
        entry.values[1].is_disabled = true;
        entry.values[1].description = Some("Unavailable: set ANTHROPIC_API_KEY".to_string());
        entry.values[1].name = "Disabled Claude".to_string();

        let mut picker = ConfigPicker::from_entry(&entry).expect("picker");
        picker.update_query("disabled".to_string());
        assert!(picker.confirm_selection().is_none());
    }

    #[test]
    fn handle_key_enter_returns_apply_selection_action() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");
        picker.move_selection_down();
        let mut input = String::new();

        let outcome = picker.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut input,
        );

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        match outcome.action {
            Some(ConfigPickerAction::ApplySelection(Some(change))) => {
                assert_eq!(change.config_id, "model");
            }
            _ => panic!("expected apply selection action"),
        }
    }

    #[test]
    fn handle_key_escape_returns_close_action() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");
        let mut input = String::new();

        let outcome =
            picker.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut input);

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(outcome.action, Some(ConfigPickerAction::Close)));
    }
}
