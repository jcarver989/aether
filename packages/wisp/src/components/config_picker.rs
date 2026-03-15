use crate::components::config_menu::{ConfigChange, ConfigMenuEntry, ConfigMenuValue};
use crate::tui::{
    Combobox, Component, Event, Frame, Line, PickerKey, Searchable, ViewContext, classify_key,
};
impl Searchable for ConfigMenuValue {
    fn search_text(&self) -> String {
        format!("{} {}", self.name, self.value)
    }
}

pub struct ConfigPicker {
    pub config_id: String,
    pub title: String,
    combobox: Combobox<ConfigMenuValue>,
    current_value: String,
}

pub enum ConfigPickerMessage {
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
        let initial_index = picker
            .combobox
            .matches()
            .iter()
            .position(|m| m.value == picker.current_value)
            .unwrap_or(0);
        picker.combobox.set_selected_index(initial_index);
        picker.ensure_selectable();
        Some(picker)
    }

    pub fn query(&self) -> &str {
        self.combobox.query()
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

    fn move_selection_up(&mut self) {
        self.combobox.move_up_where(|m| !m.is_disabled);
    }

    fn move_selection_down(&mut self) {
        self.combobox.move_down_where(|m| !m.is_disabled);
    }

    fn push_query_char(&mut self, c: char) {
        self.combobox.push_query_char(c);
        self.ensure_selectable();
    }

    fn pop_query_char(&mut self) {
        self.combobox.pop_query_char();
        self.ensure_selectable();
    }

    fn ensure_selectable(&mut self) {
        if self.combobox.is_empty() {
            return;
        }
        let idx = self.combobox.selected_index();
        if idx >= self.combobox.matches().len() || self.combobox.matches()[idx].is_disabled {
            self.combobox.select_first_where(|m| !m.is_disabled);
        }
    }
}

impl ConfigPicker {
    pub(crate) fn update_viewport(&mut self, max_height: usize) {
        self.combobox
            .set_max_visible(max_height.saturating_sub(1).max(1));
    }
}

impl Component for ConfigPicker {
    type Message = ConfigPickerMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(key) = event else {
            return None;
        };
        match classify_key(*key, self.combobox.query().is_empty()) {
            PickerKey::Escape => Some(vec![ConfigPickerMessage::Close]),
            PickerKey::MoveUp => {
                self.move_selection_up();
                Some(vec![])
            }
            PickerKey::MoveDown => {
                self.move_selection_down();
                Some(vec![])
            }
            PickerKey::Confirm => {
                let change = self.confirm_selection();
                Some(vec![ConfigPickerMessage::ApplySelection(change)])
            }
            PickerKey::Char(c) => {
                self.push_query_char(c);
                Some(vec![])
            }
            PickerKey::Backspace => {
                self.pop_query_char();
                Some(vec![])
            }
            PickerKey::MoveLeft
            | PickerKey::MoveRight
            | PickerKey::BackspaceOnEmpty
            | PickerKey::ControlChar
            | PickerKey::Other => Some(vec![]),
        }
    }

    fn render(&self, context: &ViewContext) -> Frame {
        let mut lines = Vec::new();
        let header = format!("  {} search: {}", self.title, self.combobox.query());
        lines.push(Line::styled(header, context.theme.muted()));

        if self.combobox.is_empty() {
            lines.push(Line::new("  (no matches found)".to_string()));
            return Frame::new(lines);
        }

        let item_lines = self
            .combobox
            .render_items(context, |option, is_selected, ctx| {
                let prefix = if is_selected { "▶ " } else { "  " };
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

                let line_text = format!("{prefix}{label}");
                if option.is_disabled {
                    Line::styled(line_text, ctx.theme.muted())
                } else if is_selected {
                    Line::with_style(line_text, ctx.theme.selected_row_style())
                } else {
                    Line::new(line_text)
                }
            });
        lines.extend(item_lines);

        Frame::new(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_picker::{rendered_lines_from, type_query};
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};
    use acp_utils::config_meta::SelectOptionMeta;

    fn rendered_lines(picker: &ConfigPicker) -> Vec<String> {
        rendered_lines_from(&picker.render(&ViewContext::new((120, 40))))
    }

    fn entry() -> ConfigMenuEntry {
        ConfigMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            multi_select: false,
            display_name: None,
            values: vec![
                ConfigMenuValue {
                    value: "openrouter:openai/gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "openrouter:anthropic/claude-3.5-sonnet".to_string(),
                    name: "Claude Sonnet".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "openrouter:google/gemini-2.5-pro".to_string(),
                    name: "Gemini 2.5 Pro".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
            ],
            current_value_index: 0,
            current_raw_value: "openrouter:openai/gpt-4o".to_string(),
            entry_kind: crate::components::config_menu::ConfigMenuEntryKind::Select,
        }
    }

    #[test]
    fn initializes_with_current_value_selected() {
        let picker = ConfigPicker::from_entry(&entry()).expect("picker");
        let lines = rendered_lines(&picker);
        let selected = lines.iter().find(|l| l.starts_with("▶")).unwrap();
        assert!(selected.contains("GPT-4o"));
    }

    #[test]
    fn query_filters_by_name() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");
        type_query(&mut picker, "gemini");
        let lines = rendered_lines(&picker);
        // header + 1 match
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("Gemini 2.5 Pro"));
    }

    #[test]
    fn query_filters_by_value() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");
        type_query(&mut picker, "anthropic/claude");
        let lines = rendered_lines(&picker);
        // header + 1 match
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("Claude Sonnet"));
    }

    #[test]
    fn confirm_selection_omits_unchanged_value() {
        let picker = ConfigPicker::from_entry(&entry()).expect("picker");
        assert!(picker.confirm_selection().is_none());
    }

    #[test]
    fn confirm_selection_returns_change_for_new_value() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");
        picker.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Down,
            KeyModifiers::NONE,
        )));
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
        type_query(&mut picker, "disabled");
        assert!(picker.confirm_selection().is_none());
    }

    #[test]
    fn handle_key_enter_returns_apply_selection_message() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");
        picker.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Down,
            KeyModifiers::NONE,
        )));

        let outcome = picker.on_event(&Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));

        assert!(outcome.is_some());

        let messages = outcome.unwrap();
        match messages.as_slice() {
            [ConfigPickerMessage::ApplySelection(Some(change))] => {
                assert_eq!(change.config_id, "model");
            }
            _ => panic!("expected apply selection message"),
        }
    }

    #[test]
    fn handle_key_escape_returns_close_message() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");

        let outcome = picker.on_event(&Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));

        assert!(outcome.is_some());

        let messages = outcome.unwrap();
        assert!(matches!(messages.as_slice(), [ConfigPickerMessage::Close]));
    }
}
