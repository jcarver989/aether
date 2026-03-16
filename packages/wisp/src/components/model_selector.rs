use crate::components::reasoning_bar::reasoning_bar;
use crate::settings::types::{SettingsChange, SettingsMenuEntry};
use crate::tui::{
    Combobox, Component, Event, Frame, Line, PickerKey, Searchable, ViewContext, classify_key,
};
use acp_utils::config_option_id::ConfigOptionId;
use std::cmp::Ordering;
use std::collections::HashSet;
use utils::ReasoningEffort;

#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub value: String,
    pub name: String,
    pub is_disabled: bool,
    pub supports_reasoning: bool,
}

impl ModelEntry {
    fn provider_key(&self) -> &str {
        self.value
            .split_once(':')
            .map_or("Other", |(provider, _)| provider)
    }

    fn provider_label(&self) -> String {
        if let Some((provider, _)) = self.name.split_once(" / ") {
            return provider.to_string();
        }

        let key = self.provider_key();
        if key.is_empty() {
            return "Other".to_string();
        }

        let mut chars = key.chars();
        let first = chars
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        let rest = chars.as_str().to_lowercase();
        format!("{first}{rest}")
    }

    fn model_label(&self) -> &str {
        self.name
            .split_once(" / ")
            .map_or(self.name.as_str(), |(_, model)| model)
    }
}

impl Searchable for ModelEntry {
    fn search_text(&self) -> String {
        format!("{} {}", self.name, self.value)
    }
}

fn compare_model_entries(a: &ModelEntry, b: &ModelEntry) -> Ordering {
    a.provider_key()
        .cmp(b.provider_key())
        .then_with(|| a.model_label().cmp(b.model_label()))
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.value.cmp(&b.value))
}

pub struct ModelSelector {
    combobox: Combobox<ModelEntry>,
    all_items: Vec<ModelEntry>,
    selected_models: HashSet<String>,
    original_models: HashSet<String>,
    config_id: String,
    reasoning_effort: Option<ReasoningEffort>,
    original_reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Debug)]
pub enum ModelSelectorMessage {
    Done(Vec<SettingsChange>),
}

impl ModelSelector {
    pub fn from_model_entry(
        entry: &SettingsMenuEntry,
        current_selection: Option<&str>,
        current_reasoning_effort: Option<&str>,
    ) -> Self {
        let items: Vec<ModelEntry> = entry
            .values
            .iter()
            .filter(|v| !v.is_disabled)
            .map(|v| ModelEntry {
                value: v.value.clone(),
                name: v.name.clone(),
                is_disabled: v.is_disabled,
                supports_reasoning: v.meta.supports_reasoning,
            })
            .collect();

        let selected_models: HashSet<String> = current_selection
            .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
            .unwrap_or_default();

        let reasoning = current_reasoning_effort.and_then(|s| s.parse().ok());

        let original_models = selected_models.clone();
        let all_items = items.clone();
        let mut combobox = Combobox::new(items);
        combobox.set_match_sort(compare_model_entries);
        if !selected_models.is_empty() {
            combobox.select_first_where(|item| selected_models.contains(&item.value));
        }
        Self {
            combobox,
            all_items,
            selected_models,
            original_models,
            config_id: entry.config_id.clone(),
            reasoning_effort: reasoning,
            original_reasoning_effort: reasoning,
        }
    }

    pub fn query(&self) -> &str {
        self.combobox.query()
    }

    #[cfg(test)]
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

    fn confirm(&self) -> Vec<SettingsChange> {
        let mut changes = Vec::new();
        if !self.selected_models.is_empty() && self.selected_models != self.original_models {
            let joined = self
                .selected_models
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",");
            changes.push(SettingsChange {
                config_id: self.config_id.clone(),
                new_value: joined,
            });
        }
        if self.reasoning_effort != self.original_reasoning_effort {
            changes.push(SettingsChange {
                config_id: ConfigOptionId::ReasoningEffort.as_str().to_string(),
                new_value: reasoning_config_value(self.reasoning_effort).to_string(),
            });
        }
        changes
    }
}

impl ModelSelector {
    pub fn update_viewport(&mut self, max_height: usize) {
        let overhead = if self.selected_models.is_empty() {
            2
        } else {
            4
        };
        self.combobox
            .set_max_visible(max_height.saturating_sub(overhead).max(1));
    }
}

impl Component for ModelSelector {
    type Message = ModelSelectorMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(key) = event else {
            return None;
        };
        match classify_key(*key, self.combobox.query().is_empty()) {
            PickerKey::Escape => {
                let changes = self.confirm();
                Some(vec![ModelSelectorMessage::Done(changes)])
            }
            PickerKey::MoveUp => {
                self.combobox.move_up_where(|e| !e.is_disabled);
                Some(vec![])
            }
            PickerKey::MoveDown => {
                self.combobox.move_down_where(|e| !e.is_disabled);
                Some(vec![])
            }
            PickerKey::MoveLeft => {
                if self
                    .combobox
                    .selected()
                    .is_some_and(|e| e.supports_reasoning)
                {
                    self.reasoning_effort = cycle_reasoning_left(self.reasoning_effort);
                }
                Some(vec![])
            }
            PickerKey::MoveRight => {
                if self
                    .combobox
                    .selected()
                    .is_some_and(|e| e.supports_reasoning)
                {
                    self.reasoning_effort = cycle_reasoning_right(self.reasoning_effort);
                }
                Some(vec![])
            }
            PickerKey::Confirm | PickerKey::Char(' ') => {
                self.toggle_focused();
                Some(vec![])
            }
            PickerKey::Char(c) => {
                self.combobox.push_query_char(c);
                Some(vec![])
            }
            PickerKey::Backspace => {
                self.combobox.pop_query_char();
                Some(vec![])
            }
            PickerKey::BackspaceOnEmpty | PickerKey::ControlChar | PickerKey::Other => Some(vec![]),
        }
    }

    fn render(&mut self, context: &ViewContext) -> Frame {
        let mut lines = Vec::new();
        let header = format!("  Model search: {}", self.combobox.query());
        lines.push(Line::new(header));
        lines.push(Line::new(String::new()));

        if !self.selected_models.is_empty() {
            let names: Vec<&str> = self
                .all_items
                .iter()
                .filter(|item| self.selected_models.contains(&item.value))
                .map(|item| item.name.as_str())
                .collect();
            let selected_text = format!("  Selected: {}", names.join(", "));
            lines.push(Line::styled(selected_text, context.theme.muted()));
            lines.push(Line::new(String::new()));
        }

        let mut item_lines = Vec::new();
        if self.combobox.is_empty() {
            item_lines.push(Line::new("  (no matches found)".to_string()));
        } else {
            let selected = &self.selected_models;
            let mut last_provider: Option<&str> = None;

            let items = self.combobox.visible_matches_with_selection();

            for (entry, is_focused) in &items {
                let provider = entry.provider_key();
                if last_provider != Some(provider) {
                    if !item_lines.is_empty() {
                        item_lines.push(Line::new(String::new()));
                    }
                    item_lines.push(Line::styled(
                        format!("  {}", entry.provider_label()),
                        context.theme.text_secondary(),
                    ));
                    last_provider = Some(provider);
                }

                let check = if selected.contains(&entry.value) {
                    "[x] "
                } else {
                    "[ ] "
                };
                let prefix = if *is_focused { "▶ " } else { "  " };
                let label = format!("{prefix}{check}{}", entry.model_label());

                if entry.is_disabled {
                    item_lines.push(Line::styled(label, context.theme.muted()));
                } else if *is_focused {
                    let mut line = Line::with_style(label, context.theme.selected_row_style());
                    if entry.supports_reasoning {
                        let bar = reasoning_bar(self.reasoning_effort);
                        line.push_with_style(
                            format!("    {bar}"),
                            context
                                .theme
                                .selected_row_style_with_fg(context.theme.success()),
                        );
                        line.push_with_style(
                            " reasoning",
                            context
                                .theme
                                .selected_row_style_with_fg(context.theme.text_secondary()),
                        );
                    }
                    item_lines.push(line);
                } else {
                    item_lines.push(Line::new(label));
                }
            }
        }

        let max_h = context.size.height as usize;
        let available_for_items = max_h.saturating_sub(lines.len());
        item_lines.truncate(available_for_items);
        lines.extend(item_lines);

        Frame::new(lines)
    }
}

#[allow(clippy::unnecessary_wraps)]
fn cycle_reasoning_right(effort: Option<ReasoningEffort>) -> Option<ReasoningEffort> {
    match effort {
        None => Some(ReasoningEffort::Low),
        Some(ReasoningEffort::Low) => Some(ReasoningEffort::Medium),
        Some(ReasoningEffort::Medium | ReasoningEffort::High) => Some(ReasoningEffort::High),
    }
}

fn cycle_reasoning_left(effort: Option<ReasoningEffort>) -> Option<ReasoningEffort> {
    match effort {
        None | Some(ReasoningEffort::Low) => None,
        Some(ReasoningEffort::Medium) => Some(ReasoningEffort::Low),
        Some(ReasoningEffort::High) => Some(ReasoningEffort::Medium),
    }
}

fn reasoning_config_value(effort: Option<ReasoningEffort>) -> &'static str {
    ReasoningEffort::config_str(effort)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::types::{SettingsMenuEntryKind, SettingsMenuValue};
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};
    use acp_utils::config_meta::SelectOptionMeta;

    fn model_entry() -> SettingsMenuEntry {
        SettingsMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            values: vec![
                SettingsMenuValue {
                    value: "anthropic:claude-sonnet-4-5".to_string(),
                    name: "Anthropic / Claude Sonnet 4.5".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                SettingsMenuValue {
                    value: "deepseek:deepseek-chat".to_string(),
                    name: "DeepSeek / DeepSeek Chat".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                SettingsMenuValue {
                    value: "gemini:gemini-2.5-pro".to_string(),
                    name: "Google / Gemini 2.5 Pro".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
            ],
            current_value_index: 0,
            current_raw_value: "anthropic:claude-sonnet-4-5".to_string(),
            entry_kind: SettingsMenuEntryKind::Select,
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

    #[tokio::test]
    async fn toggle_adds_and_removes_model() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        assert_eq!(builder.selected_count(), 0);

        builder.on_event(&Event::Key(space())).await; // toggle first
        assert_eq!(builder.selected_count(), 1);

        builder.on_event(&Event::Key(space())).await; // toggle first again
        assert_eq!(builder.selected_count(), 0);
    }

    #[test]
    fn confirm_with_zero_returns_empty() {
        let builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        assert!(builder.confirm().is_empty());
    }

    #[tokio::test]
    async fn confirm_with_one_returns_single_model() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        builder.on_event(&Event::Key(space())).await; // select first
        let changes = builder.confirm();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].config_id, "model");
        assert_eq!(changes[0].new_value, "anthropic:claude-sonnet-4-5");
    }

    #[tokio::test]
    async fn confirm_with_two_returns_comma_joined() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        builder.on_event(&Event::Key(space())).await; // select first
        builder.on_event(&Event::Key(key(KeyCode::Down))).await;
        builder.on_event(&Event::Key(space())).await; // select second

        let changes = builder.confirm();
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
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
            None,
        );
        assert_eq!(builder.selected_count(), 2);
    }

    #[tokio::test]
    async fn escape_returns_done_action() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        let outcome = builder.on_event(&Event::Key(key(KeyCode::Esc))).await;
        let messages = outcome.unwrap();
        match messages.as_slice() {
            [ModelSelectorMessage::Done(changes)] => assert!(changes.is_empty()),
            other => panic!("expected Done([]), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn enter_toggles_focused_model() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        assert_eq!(builder.selected_count(), 0);

        builder.on_event(&Event::Key(key(KeyCode::Enter))).await; // toggle first
        assert_eq!(builder.selected_count(), 1);

        builder.on_event(&Event::Key(key(KeyCode::Enter))).await; // toggle first again
        assert_eq!(builder.selected_count(), 0);
    }

    #[tokio::test]
    async fn escape_with_selections_returns_done_with_change() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        builder.on_event(&Event::Key(space())).await; // select first
        builder.on_event(&Event::Key(key(KeyCode::Down))).await;
        builder.on_event(&Event::Key(space())).await; // select second

        let outcome = builder.on_event(&Event::Key(key(KeyCode::Esc))).await;
        let messages = outcome.unwrap();
        match messages.as_slice() {
            [ModelSelectorMessage::Done(changes)] => {
                assert_eq!(changes.len(), 1);
                let change = &changes[0];
                assert_eq!(change.config_id, "model");
                let parts: HashSet<&str> = change.new_value.split(',').collect();
                assert!(parts.contains("anthropic:claude-sonnet-4-5"));
                assert!(parts.contains("deepseek:deepseek-chat"));
            }
            other => panic!("expected Done with model change, got: {other:?}"),
        }
    }

    #[test]
    fn escape_without_toggle_returns_no_change() {
        let builder = ModelSelector::from_model_entry(
            &model_entry(),
            Some("anthropic:claude-sonnet-4-5,deepseek:deepseek-chat"),
            None,
        );
        // No toggling — confirm should return empty since selection == original
        assert!(builder.confirm().is_empty());
    }

    #[tokio::test]
    async fn escape_after_toggle_returns_change() {
        let mut builder = ModelSelector::from_model_entry(
            &model_entry(),
            Some("anthropic:claude-sonnet-4-5"),
            None,
        );
        // Toggle a second model on
        builder.on_event(&Event::Key(key(KeyCode::Down))).await;
        builder.on_event(&Event::Key(space())).await;
        let changes = builder.confirm();
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
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

        let builder = ModelSelector::from_model_entry(&entry, None, None);
        // Should only have 2 entries (disabled one filtered)
        assert_eq!(builder.combobox.matches().len(), 2);
    }

    fn reasoning_meta() -> SelectOptionMeta {
        SelectOptionMeta {
            supports_reasoning: true,
        }
    }

    fn model_entry_with_reasoning() -> SettingsMenuEntry {
        SettingsMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            values: vec![
                SettingsMenuValue {
                    value: "anthropic:claude-opus-4-6".to_string(),
                    name: "Anthropic / Claude Opus 4.6".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: reasoning_meta(),
                },
                SettingsMenuValue {
                    value: "deepseek:deepseek-chat".to_string(),
                    name: "DeepSeek / DeepSeek Chat".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
            ],
            current_value_index: 0,
            current_raw_value: "anthropic:claude-opus-4-6".to_string(),
            entry_kind: SettingsMenuEntryKind::Select,
            multi_select: true,
            display_name: None,
        }
    }

    #[test]
    fn reasoning_cycle_right_clamps_at_high() {
        use ReasoningEffort::*;
        assert_eq!(cycle_reasoning_right(None), Some(Low));
        assert_eq!(cycle_reasoning_right(Some(Low)), Some(Medium));
        assert_eq!(cycle_reasoning_right(Some(Medium)), Some(High));
        assert_eq!(cycle_reasoning_right(Some(High)), Some(High));
    }

    #[test]
    fn reasoning_cycle_left_clamps_at_none() {
        use ReasoningEffort::*;
        assert_eq!(cycle_reasoning_left(Some(High)), Some(Medium));
        assert_eq!(cycle_reasoning_left(Some(Medium)), Some(Low));
        assert_eq!(cycle_reasoning_left(Some(Low)), None);
        assert_eq!(cycle_reasoning_left(None), None);
    }

    #[tokio::test]
    async fn right_on_reasoning_model_cycles_level() {
        let mut selector =
            ModelSelector::from_model_entry(&model_entry_with_reasoning(), None, None);
        assert_eq!(selector.reasoning_effort, None);

        selector.on_event(&Event::Key(key(KeyCode::Right))).await;
        assert_eq!(selector.reasoning_effort, Some(ReasoningEffort::Low));

        selector.on_event(&Event::Key(key(KeyCode::Right))).await;
        assert_eq!(selector.reasoning_effort, Some(ReasoningEffort::Medium));
    }

    #[tokio::test]
    async fn left_right_on_non_reasoning_model_is_noop() {
        let mut selector =
            ModelSelector::from_model_entry(&model_entry_with_reasoning(), None, None);
        // Move to non-reasoning model (DeepSeek)
        selector.on_event(&Event::Key(key(KeyCode::Down))).await;
        assert!(!selector.combobox.selected().unwrap().supports_reasoning);

        selector.on_event(&Event::Key(key(KeyCode::Right))).await;
        assert_eq!(selector.reasoning_effort, None);
    }

    #[tokio::test]
    async fn confirm_returns_both_model_and_reasoning_changes() {
        let mut selector =
            ModelSelector::from_model_entry(&model_entry_with_reasoning(), None, None);
        // Toggle a model on
        selector.on_event(&Event::Key(space())).await;
        // Change reasoning
        selector.on_event(&Event::Key(key(KeyCode::Right))).await;

        let changes = selector.confirm();
        assert_eq!(changes.len(), 2, "expected model + reasoning changes");
        assert!(changes.iter().any(|c| c.config_id == "model"));
        assert!(
            changes
                .iter()
                .any(|c| c.config_id == "reasoning_effort" && c.new_value == "low")
        );
    }

    #[tokio::test]
    async fn confirm_returns_only_reasoning_when_only_reasoning_changed() {
        let mut selector = ModelSelector::from_model_entry(
            &model_entry_with_reasoning(),
            Some("anthropic:claude-opus-4-6"),
            None,
        );
        // Don't change models, just reasoning
        selector.on_event(&Event::Key(key(KeyCode::Right))).await;
        selector.on_event(&Event::Key(key(KeyCode::Right))).await;

        let changes = selector.confirm();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].config_id, "reasoning_effort");
        assert_eq!(changes[0].new_value, "medium");
    }

    #[test]
    fn confirm_returns_empty_when_nothing_changed() {
        let selector = ModelSelector::from_model_entry(
            &model_entry_with_reasoning(),
            Some("anthropic:claude-opus-4-6"),
            Some("high"),
        );
        assert!(selector.confirm().is_empty());
    }
}
