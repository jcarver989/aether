use crate::components::config_menu::{ConfigChange, ConfigMenuEntry};
use crate::tui::{
    Combobox, Component, HandlesInput, InputOutcome, Line, PickerKey, RenderContext, Searchable,
    Style, classify_key,
};
use crossterm::event::KeyEvent;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub value: String,
    pub name: String,
    pub is_disabled: bool,
}

impl Searchable for ModelEntry {
    fn search_text(&self) -> String {
        format!("{} {}", self.name, self.value)
    }
}

pub struct ModelSelector {
    combobox: Combobox<ModelEntry>,
    all_items: Vec<ModelEntry>,
    selected_models: HashSet<String>,
    original_models: HashSet<String>,
    config_id: String,
}

#[derive(Debug)]
pub enum ModelSelectorAction {
    Done(Option<ConfigChange>),
}

impl ModelSelector {
    pub fn from_model_entry(entry: &ConfigMenuEntry, current_selection: Option<&str>) -> Self {
        let items: Vec<ModelEntry> = entry
            .values
            .iter()
            .filter(|v| !v.is_disabled)
            .map(|v| ModelEntry {
                value: v.value.clone(),
                name: v.name.clone(),
                is_disabled: v.is_disabled,
            })
            .collect();

        let selected_models: HashSet<String> = current_selection
            .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
            .unwrap_or_default();

        let original_models = selected_models.clone();
        let all_items = items.clone();
        Self {
            combobox: Combobox::new(items),
            all_items,
            selected_models,
            original_models,
            config_id: entry.config_id.clone(),
        }
    }

    pub fn query(&self) -> &str {
        self.combobox.query()
    }

    #[allow(dead_code)]
    pub fn selected_count(&self) -> usize {
        self.selected_models.len()
    }

    fn toggle_focused(&mut self) {
        if let Some(entry) = self.combobox.selected() {
            if entry.is_disabled {
                return;
            }
            let value = entry.value.clone();
            if !self.selected_models.remove(&value) {
                self.selected_models.insert(value);
            }
        }
    }

    fn confirm(&self) -> Option<ConfigChange> {
        if self.selected_models.is_empty() || self.selected_models == self.original_models {
            return None;
        }
        let joined = self
            .selected_models
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        Some(ConfigChange {
            config_id: self.config_id.clone(),
            new_value: joined,
        })
    }
}

impl Component for ModelSelector {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        if let Some(h) = context.max_height {
            // Overhead: header (1) + optional "Selected:" line (0-1) + footer (1)
            let has_selected_line = !self.selected_models.is_empty();
            let overhead = 1 + usize::from(has_selected_line) + 1;
            self.combobox
                .set_max_visible(h.saturating_sub(overhead).max(1));
        }

        let mut lines = Vec::new();
        let header = format!("  Model search: {}", self.combobox.query());
        lines.push(Line::styled(header, context.theme.muted));

        if !self.selected_models.is_empty() {
            let names: Vec<&str> = self
                .all_items
                .iter()
                .filter(|item| self.selected_models.contains(&item.value))
                .map(|item| item.name.as_str())
                .collect();
            let selected_text = format!("  Selected: {}", names.join(", "));
            lines.push(Line::styled(selected_text, context.theme.muted));
        }

        if self.combobox.is_empty() {
            lines.push(Line::new("  (no matches found)".to_string()));
        } else {
            let selected = &self.selected_models;
            let item_lines = self
                .combobox
                .render_items(context, |entry, is_focused, ctx| {
                    let check = if selected.contains(&entry.value) {
                        "[x] "
                    } else {
                        "[ ] "
                    };
                    let prefix = if is_focused { "▶ " } else { "  " };
                    let label = format!("{prefix}{check}{}", entry.name);

                    if entry.is_disabled {
                        Line::styled(label, ctx.theme.muted)
                    } else if is_focused {
                        Line::with_style(
                            label,
                            Style::fg(ctx.theme.text_primary).bg_color(ctx.theme.highlight_bg),
                        )
                    } else {
                        Line::new(label)
                    }
                });
            lines.extend(item_lines);
        }

        let count = self.selected_models.len();
        let footer = if count == 0 {
            "  0 selected".to_string()
        } else if count == 1 {
            "  1 model selected".to_string()
        } else {
            format!("  {count} models selected")
        };
        lines.push(Line::styled(footer, context.theme.muted));

        lines
    }
}

impl HandlesInput for ModelSelector {
    type Action = ModelSelectorAction;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action> {
        match classify_key(key_event, self.combobox.query().is_empty()) {
            PickerKey::Escape => {
                let change = self.confirm();
                InputOutcome::action_and_render(ModelSelectorAction::Done(change))
            }
            PickerKey::MoveUp => {
                self.combobox.move_up_where(|e| !e.is_disabled);
                InputOutcome::consumed_and_render()
            }
            PickerKey::MoveDown => {
                self.combobox.move_down_where(|e| !e.is_disabled);
                InputOutcome::consumed_and_render()
            }
            PickerKey::Confirm | PickerKey::Char(' ') => {
                self.toggle_focused();
                InputOutcome::consumed_and_render()
            }
            PickerKey::Char(c) => {
                self.combobox.push_query_char(c);
                InputOutcome::consumed_and_render()
            }
            PickerKey::Backspace => {
                self.combobox.pop_query_char();
                InputOutcome::consumed_and_render()
            }
            PickerKey::BackspaceOnEmpty | PickerKey::ControlChar | PickerKey::Other => {
                InputOutcome::consumed()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::config_menu::{ConfigMenuEntryKind, ConfigMenuValue};
    use crate::tui::test_picker::{rendered_lines, type_query};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn model_entry() -> ConfigMenuEntry {
        ConfigMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            values: vec![
                ConfigMenuValue {
                    value: "anthropic:claude-sonnet-4-5".to_string(),
                    name: "Anthropic / Claude Sonnet 4.5".to_string(),
                    description: None,
                    is_disabled: false,
                },
                ConfigMenuValue {
                    value: "deepseek:deepseek-chat".to_string(),
                    name: "DeepSeek / DeepSeek Chat".to_string(),
                    description: None,
                    is_disabled: false,
                },
                ConfigMenuValue {
                    value: "gemini:gemini-2.5-pro".to_string(),
                    name: "Google / Gemini 2.5 Pro".to_string(),
                    description: None,
                    is_disabled: false,
                },
            ],
            current_value_index: 0,
            current_raw_value: "anthropic:claude-sonnet-4-5".to_string(),
            entry_kind: ConfigMenuEntryKind::Select,
            multi_select: true,
            display_name: None,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn space() -> KeyEvent {
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)
    }

    #[test]
    fn toggle_adds_and_removes_model() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None);
        assert_eq!(builder.selected_count(), 0);

        builder.handle_key(space()); // toggle first
        assert_eq!(builder.selected_count(), 1);

        builder.handle_key(space()); // toggle first again
        assert_eq!(builder.selected_count(), 0);
    }

    #[test]
    fn confirm_with_zero_returns_none() {
        let builder = ModelSelector::from_model_entry(&model_entry(), None);
        assert!(builder.confirm().is_none());
    }

    #[test]
    fn confirm_with_one_returns_single_model() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None);
        builder.handle_key(space()); // select first
        let change = builder.confirm().expect("should produce a change");
        assert_eq!(change.config_id, "model");
        assert_eq!(change.new_value, "anthropic:claude-sonnet-4-5");
    }

    #[test]
    fn confirm_with_two_returns_comma_joined() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None);
        builder.handle_key(space()); // select first
        builder.handle_key(key(KeyCode::Down));
        builder.handle_key(space()); // select second

        let change = builder.confirm().expect("should produce a change");
        assert_eq!(change.config_id, "model");
        let parts: HashSet<&str> = change.new_value.split(',').collect();
        assert!(parts.contains("anthropic:claude-sonnet-4-5"));
        assert!(parts.contains("deepseek:deepseek-chat"));
    }

    #[test]
    fn pre_selected_values_from_current_selection() {
        let builder = ModelSelector::from_model_entry(
            &model_entry(),
            Some("anthropic:claude-sonnet-4-5,deepseek:deepseek-chat"),
        );
        assert_eq!(builder.selected_count(), 2);
    }

    #[test]
    fn search_filters_entries() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None);
        type_query(&mut builder, "deepseek");
        let lines = rendered_lines(&mut builder);
        // header + 1 match + footer
        assert_eq!(lines.len(), 3);
        assert!(lines[1].contains("DeepSeek"));
    }

    #[test]
    fn render_shows_checkboxes() {
        let mut builder =
            ModelSelector::from_model_entry(&model_entry(), Some("anthropic:claude-sonnet-4-5"));
        let lines = rendered_lines(&mut builder);
        // First entry should be checked
        assert!(lines.iter().any(|l| l.contains("[x]")));
        // Others unchecked
        assert!(lines.iter().any(|l| l.contains("[ ]")));
    }

    #[test]
    fn escape_returns_done_action() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None);
        let outcome = builder.handle_key(key(KeyCode::Esc));
        assert!(matches!(
            outcome.action,
            Some(ModelSelectorAction::Done(None))
        ));
    }

    #[test]
    fn enter_toggles_focused_model() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None);
        assert_eq!(builder.selected_count(), 0);

        builder.handle_key(key(KeyCode::Enter)); // toggle first
        assert_eq!(builder.selected_count(), 1);

        builder.handle_key(key(KeyCode::Enter)); // toggle first again
        assert_eq!(builder.selected_count(), 0);
    }

    #[test]
    fn escape_with_selections_returns_done_with_change() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None);
        builder.handle_key(space()); // select first
        builder.handle_key(key(KeyCode::Down));
        builder.handle_key(space()); // select second

        let outcome = builder.handle_key(key(KeyCode::Esc));
        match outcome.action {
            Some(ModelSelectorAction::Done(Some(change))) => {
                assert_eq!(change.config_id, "model");
                let parts: HashSet<&str> = change.new_value.split(',').collect();
                assert!(parts.contains("anthropic:claude-sonnet-4-5"));
                assert!(parts.contains("deepseek:deepseek-chat"));
            }
            other => panic!("expected Done(Some(change)), got: {other:?}"),
        }
    }

    #[test]
    fn render_shows_selected_models_at_top() {
        let mut builder = ModelSelector::from_model_entry(
            &model_entry(),
            Some("anthropic:claude-sonnet-4-5,deepseek:deepseek-chat"),
        );
        let lines = rendered_lines(&mut builder);
        // Second line (after header) should show selected models
        assert!(
            lines[1].contains("Selected:"),
            "expected Selected line, got: {}",
            lines[1]
        );
        assert!(lines[1].contains("Claude Sonnet 4.5"));
        assert!(lines[1].contains("DeepSeek Chat"));
    }

    #[test]
    fn render_hides_selected_line_when_none_selected() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None);
        let lines = rendered_lines(&mut builder);
        assert!(
            !lines.iter().any(|l| l.contains("Selected:")),
            "should not show Selected line when nothing is selected"
        );
    }

    #[test]
    fn escape_without_toggle_returns_no_change() {
        let builder = ModelSelector::from_model_entry(
            &model_entry(),
            Some("anthropic:claude-sonnet-4-5,deepseek:deepseek-chat"),
        );
        // No toggling — confirm should return None since selection == original
        assert!(builder.confirm().is_none());
    }

    #[test]
    fn escape_after_toggle_returns_change() {
        let mut builder =
            ModelSelector::from_model_entry(&model_entry(), Some("anthropic:claude-sonnet-4-5"));
        // Toggle a second model on
        builder.handle_key(key(KeyCode::Down));
        builder.handle_key(space());
        let change = builder.confirm().expect("should produce a change");
        assert_eq!(change.config_id, "model");
        let parts: HashSet<&str> = change.new_value.split(',').collect();
        assert!(parts.contains("anthropic:claude-sonnet-4-5"));
        assert!(parts.contains("deepseek:deepseek-chat"));
    }

    #[test]
    fn disabled_entries_filtered_from_builder() {
        let mut entry = model_entry();
        entry.values[1].is_disabled = true;
        entry.values[1].description = Some("Unavailable: set DEEPSEEK_API_KEY".to_string());

        let builder = ModelSelector::from_model_entry(&entry, None);
        // Should only have 2 entries (disabled one filtered)
        assert_eq!(builder.combobox.matches().len(), 2);
    }
}
