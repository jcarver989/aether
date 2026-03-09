use crate::components::config_menu::{ConfigChange, ConfigMenuEntry};
use crate::components::reasoning_bar::reasoning_bar;
use crate::tui::{
    Combobox, Component, InteractiveComponent, Line, MessageResult, PickerKey, RenderContext,
    Searchable, UiEvent, classify_key,
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
    Done(Vec<ConfigChange>),
}

impl ModelSelector {
    pub fn from_model_entry(
        entry: &ConfigMenuEntry,
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

    fn confirm(&self) -> Vec<ConfigChange> {
        let mut changes = Vec::new();
        if !self.selected_models.is_empty() && self.selected_models != self.original_models {
            let joined = self
                .selected_models
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",");
            changes.push(ConfigChange {
                config_id: self.config_id.clone(),
                new_value: joined,
            });
        }
        if self.reasoning_effort != self.original_reasoning_effort {
            changes.push(ConfigChange {
                config_id: ConfigOptionId::ReasoningEffort.as_str().to_string(),
                new_value: reasoning_config_value(self.reasoning_effort).to_string(),
            });
        }
        changes
    }
}

impl ModelSelector {
    pub(crate) fn prepare_render(&mut self, context: &RenderContext) {
        let has_selected_line = !self.selected_models.is_empty();
        if let Some(h) = context.max_height {
            let overhead = if has_selected_line { 4 } else { 2 };
            self.combobox
                .set_max_visible(h.saturating_sub(overhead).max(1));
        }
    }
}

impl Component for ModelSelector {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
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

        if let Some(h) = context.max_height {
            let available_for_items = h.saturating_sub(lines.len());
            item_lines.truncate(available_for_items);
        }
        lines.extend(item_lines);

        lines
    }
}

impl InteractiveComponent for ModelSelector {
    type Message = ModelSelectorMessage;

    fn on_event(&mut self, event: UiEvent) -> MessageResult<Self::Message> {
        match event {
            UiEvent::Key(key_event) => {
                match classify_key(key_event, self.combobox.query().is_empty()) {
                    PickerKey::Escape => {
                        let changes = self.confirm();
                        MessageResult::message(ModelSelectorMessage::Done(changes))
                    }
                    PickerKey::MoveUp => {
                        self.combobox.move_up_where(|e| !e.is_disabled);
                        MessageResult::consumed()
                    }
                    PickerKey::MoveDown => {
                        self.combobox.move_down_where(|e| !e.is_disabled);
                        MessageResult::consumed()
                    }
                    PickerKey::MoveLeft => {
                        if self
                            .combobox
                            .selected()
                            .is_some_and(|e| e.supports_reasoning)
                        {
                            self.reasoning_effort = cycle_reasoning_left(self.reasoning_effort);
                        }
                        MessageResult::consumed()
                    }
                    PickerKey::MoveRight => {
                        if self
                            .combobox
                            .selected()
                            .is_some_and(|e| e.supports_reasoning)
                        {
                            self.reasoning_effort = cycle_reasoning_right(self.reasoning_effort);
                        }
                        MessageResult::consumed()
                    }
                    PickerKey::Confirm | PickerKey::Char(' ') => {
                        self.toggle_focused();
                        MessageResult::consumed()
                    }
                    PickerKey::Char(c) => {
                        self.combobox.push_query_char(c);
                        MessageResult::consumed()
                    }
                    PickerKey::Backspace => {
                        self.combobox.pop_query_char();
                        MessageResult::consumed()
                    }
                    PickerKey::BackspaceOnEmpty | PickerKey::ControlChar | PickerKey::Other => {
                        MessageResult::consumed()
                    }
                }
            }
            UiEvent::Paste(_) | UiEvent::Tick(_) => MessageResult::ignored(),
        }
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
    use crate::components::config_menu::{ConfigMenuEntryKind, ConfigMenuValue};
    use crate::tui::test_picker::{rendered_lines, type_query};
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};
    use acp_utils::config_meta::SelectOptionMeta;

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
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "deepseek:deepseek-chat".to_string(),
                    name: "DeepSeek / DeepSeek Chat".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "gemini:gemini-2.5-pro".to_string(),
                    name: "Google / Gemini 2.5 Pro".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
            ],
            current_value_index: 0,
            current_raw_value: "anthropic:claude-sonnet-4-5".to_string(),
            entry_kind: ConfigMenuEntryKind::Select,
            multi_select: true,
            display_name: None,
        }
    }

    fn model_entry_with_groups() -> ConfigMenuEntry {
        ConfigMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            values: vec![
                ConfigMenuValue {
                    value: "openrouter:anthropic/claude-sonnet-4-5".to_string(),
                    name: "OpenRouter / Claude Sonnet 4.5".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "openrouter:google/gemini-2.5-pro".to_string(),
                    name: "OpenRouter / Gemini 2.5 Pro".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "anthropic:claude-sonnet-4-5".to_string(),
                    name: "Anthropic / Claude Sonnet 4.5".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "gemini:gemini-2.5-pro".to_string(),
                    name: "Google / Gemini 2.5 Pro".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
            ],
            current_value_index: 0,
            current_raw_value: "openrouter:anthropic/claude-sonnet-4-5".to_string(),
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
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        assert_eq!(builder.selected_count(), 0);

        builder.on_event(UiEvent::Key(space())); // toggle first
        assert_eq!(builder.selected_count(), 1);

        builder.on_event(UiEvent::Key(space())); // toggle first again
        assert_eq!(builder.selected_count(), 0);
    }

    #[test]
    fn confirm_with_zero_returns_empty() {
        let builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        assert!(builder.confirm().is_empty());
    }

    #[test]
    fn confirm_with_one_returns_single_model() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        builder.on_event(UiEvent::Key(space())); // select first
        let changes = builder.confirm();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].config_id, "model");
        assert_eq!(changes[0].new_value, "anthropic:claude-sonnet-4-5");
    }

    #[test]
    fn confirm_with_two_returns_comma_joined() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        builder.on_event(UiEvent::Key(space())); // select first
        builder.on_event(UiEvent::Key(key(KeyCode::Down)));
        builder.on_event(UiEvent::Key(space())); // select second

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

    #[test]
    fn search_filters_entries() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        type_query(&mut builder, "deepseek");
        let lines = rendered_lines(&mut builder);
        assert!(lines.iter().any(|l| l.trim() == "DeepSeek"));
        assert!(lines.iter().any(|l| l.contains("[ ] DeepSeek Chat")));
    }

    #[test]
    fn render_groups_models_under_provider_headers() {
        let mut builder = ModelSelector::from_model_entry(&model_entry_with_groups(), None, None);
        let lines = rendered_lines(&mut builder);

        let openrouter_headers = lines.iter().filter(|l| l.trim() == "OpenRouter").count();
        assert_eq!(openrouter_headers, 1, "expected one OpenRouter header line");
        assert!(
            lines
                .windows(2)
                .any(|w| w[0].trim().is_empty() && w[1].trim() == "Anthropic"),
            "expected blank separator before next provider: {lines:?}"
        );
        assert!(lines.iter().any(|l| l.contains("[ ] Claude Sonnet 4.5")));
        assert!(lines.iter().any(|l| l.contains("[ ] Gemini 2.5 Pro")));
    }

    #[test]
    fn search_filters_and_keeps_provider_headers() {
        let mut builder = ModelSelector::from_model_entry(&model_entry_with_groups(), None, None);
        type_query(&mut builder, "gemini");
        let lines = rendered_lines(&mut builder);

        assert!(
            lines.iter().any(|l| l.trim() == "OpenRouter"),
            "missing OpenRouter header in filtered results: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.trim() == "Google"),
            "missing Google header in filtered results: {lines:?}"
        );
        assert!(lines.iter().any(|l| l.contains("[ ] Gemini 2.5 Pro")));
    }

    #[test]
    fn search_does_not_duplicate_provider_headers() {
        let entry = ConfigMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            values: vec![
                ConfigMenuValue {
                    value: "codex:gpt-5".to_string(),
                    name: "Codex / GPT-5".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "openrouter:gpt-5".to_string(),
                    name: "OpenRouter / GPT-5".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "codex:gpt-5-mini".to_string(),
                    name: "Codex / GPT-5 Mini".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
                ConfigMenuValue {
                    value: "openrouter:gpt-5-mini".to_string(),
                    name: "OpenRouter / GPT-5 Mini".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
            ],
            current_value_index: 0,
            current_raw_value: "codex:gpt-5".to_string(),
            entry_kind: ConfigMenuEntryKind::Select,
            multi_select: true,
            display_name: None,
        };
        let mut selector = ModelSelector::from_model_entry(&entry, None, None);
        type_query(&mut selector, "gpt");
        let lines = rendered_lines(&mut selector);

        let codex_count = lines.iter().filter(|l| l.trim() == "Codex").count();
        let openrouter_count = lines.iter().filter(|l| l.trim() == "OpenRouter").count();
        assert_eq!(
            codex_count, 1,
            "expected exactly one Codex header, got {codex_count}: {lines:?}"
        );
        assert_eq!(
            openrouter_count, 1,
            "expected exactly one OpenRouter header, got {openrouter_count}: {lines:?}"
        );
    }

    fn focused_provider_and_row(selector: &mut ModelSelector) -> (String, String) {
        let lines = rendered_lines(selector);
        let focused_idx = lines
            .iter()
            .position(|line| line.starts_with("▶"))
            .expect("should have focused row");
        let provider = lines[..focused_idx]
            .iter()
            .rev()
            .map(|line| line.trim())
            .find(|line| {
                !line.is_empty()
                    && !line.contains("Model search:")
                    && !line.contains("Selected:")
                    && !line.starts_with('[')
                    && !line.starts_with('▶')
            })
            .expect("should find provider header")
            .to_string();

        (provider, lines[focused_idx].clone())
    }

    #[test]
    fn grouped_navigation_follows_rendered_order() {
        let mut selector = ModelSelector::from_model_entry(&model_entry_with_groups(), None, None);

        let (provider, focused) = focused_provider_and_row(&mut selector);
        assert_eq!(provider, "Anthropic");
        assert!(focused.contains("Claude Sonnet 4.5"));

        selector.on_event(UiEvent::Key(key(KeyCode::Down)));
        let (provider, focused) = focused_provider_and_row(&mut selector);
        assert_eq!(provider, "Google");
        assert!(focused.contains("Gemini 2.5 Pro"));

        selector.on_event(UiEvent::Key(key(KeyCode::Down)));
        let (provider, focused) = focused_provider_and_row(&mut selector);
        assert_eq!(provider, "OpenRouter");
        assert!(focused.contains("Claude Sonnet 4.5"));
    }

    #[test]
    fn grouped_navigation_after_search_follows_rendered_order() {
        let mut selector = ModelSelector::from_model_entry(&model_entry_with_groups(), None, None);
        type_query(&mut selector, "2.5");

        let (provider, focused) = focused_provider_and_row(&mut selector);
        assert_eq!(provider, "Google");
        assert!(focused.contains("Gemini 2.5 Pro"));

        selector.on_event(UiEvent::Key(key(KeyCode::Down)));
        let (provider, focused) = focused_provider_and_row(&mut selector);
        assert_eq!(provider, "OpenRouter");
        assert!(focused.contains("Gemini 2.5 Pro"));
    }

    #[test]
    fn grouped_render_respects_small_height() {
        let mut builder = ModelSelector::from_model_entry(&model_entry_with_groups(), None, None);
        let context = RenderContext::new((120, 40)).with_max_height(6);
        builder.prepare_render(&context);
        let lines: Vec<String> = builder
            .render(&context)
            .iter()
            .map(Line::plain_text)
            .collect();

        assert!(
            lines.len() <= 6,
            "rendered too many lines for viewport: {lines:?}"
        );
        assert!(
            !lines
                .iter()
                .any(|l| l.contains("model selected") || l.contains("selected")),
            "did not expect bottom selected-count footer: {lines:?}"
        );
    }

    #[test]
    fn escape_returns_done_action() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        let outcome = builder.on_event(UiEvent::Key(key(KeyCode::Esc)));
        match outcome.messages.as_slice() {
            [ModelSelectorMessage::Done(changes)] => assert!(changes.is_empty()),
            other => panic!("expected Done([]), got: {other:?}"),
        }
    }

    #[test]
    fn enter_toggles_focused_model() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        assert_eq!(builder.selected_count(), 0);

        builder.on_event(UiEvent::Key(key(KeyCode::Enter))); // toggle first
        assert_eq!(builder.selected_count(), 1);

        builder.on_event(UiEvent::Key(key(KeyCode::Enter))); // toggle first again
        assert_eq!(builder.selected_count(), 0);
    }

    #[test]
    fn escape_with_selections_returns_done_with_change() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        builder.on_event(UiEvent::Key(space())); // select first
        builder.on_event(UiEvent::Key(key(KeyCode::Down)));
        builder.on_event(UiEvent::Key(space())); // select second

        let outcome = builder.on_event(UiEvent::Key(key(KeyCode::Esc)));
        match outcome.messages.as_slice() {
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
    fn render_shows_selected_models_at_top() {
        let mut builder = ModelSelector::from_model_entry(
            &model_entry(),
            Some("anthropic:claude-sonnet-4-5,deepseek:deepseek-chat"),
            None,
        );
        let lines = rendered_lines(&mut builder);
        // Second line after header should be a spacer, then selected models line
        assert!(
            lines[1].trim().is_empty(),
            "expected spacer line after header"
        );
        assert!(
            lines[2].contains("Selected:"),
            "expected Selected line, got: {}",
            lines[2]
        );
        assert!(lines[2].contains("Claude Sonnet 4.5"));
        assert!(lines[2].contains("DeepSeek Chat"));
        assert!(
            lines.get(3).is_some_and(|l| l.trim().is_empty()),
            "expected spacer line after selected line"
        );
    }

    #[test]
    fn render_hides_selected_line_when_none_selected() {
        let mut builder = ModelSelector::from_model_entry(&model_entry(), None, None);
        let lines = rendered_lines(&mut builder);
        assert!(
            !lines.iter().any(|l| l.contains("Selected:")),
            "should not show Selected line when nothing is selected"
        );
        assert!(
            lines.get(1).is_some_and(|l| l.trim().is_empty()),
            "expected blank line after search header"
        );
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

    #[test]
    fn escape_after_toggle_returns_change() {
        let mut builder = ModelSelector::from_model_entry(
            &model_entry(),
            Some("anthropic:claude-sonnet-4-5"),
            None,
        );
        // Toggle a second model on
        builder.on_event(UiEvent::Key(key(KeyCode::Down)));
        builder.on_event(UiEvent::Key(space()));
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

    fn model_entry_with_reasoning() -> ConfigMenuEntry {
        ConfigMenuEntry {
            config_id: "model".to_string(),
            title: "Model".to_string(),
            values: vec![
                ConfigMenuValue {
                    value: "anthropic:claude-opus-4-6".to_string(),
                    name: "Anthropic / Claude Opus 4.6".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: reasoning_meta(),
                },
                ConfigMenuValue {
                    value: "deepseek:deepseek-chat".to_string(),
                    name: "DeepSeek / DeepSeek Chat".to_string(),
                    description: None,
                    is_disabled: false,
                    meta: SelectOptionMeta::default(),
                },
            ],
            current_value_index: 0,
            current_raw_value: "anthropic:claude-opus-4-6".to_string(),
            entry_kind: ConfigMenuEntryKind::Select,
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

    #[test]
    fn right_on_reasoning_model_cycles_level() {
        let mut selector =
            ModelSelector::from_model_entry(&model_entry_with_reasoning(), None, None);
        assert_eq!(selector.reasoning_effort, None);

        selector.on_event(UiEvent::Key(key(KeyCode::Right)));
        assert_eq!(selector.reasoning_effort, Some(ReasoningEffort::Low));

        selector.on_event(UiEvent::Key(key(KeyCode::Right)));
        assert_eq!(selector.reasoning_effort, Some(ReasoningEffort::Medium));
    }

    #[test]
    fn left_right_on_non_reasoning_model_is_noop() {
        let mut selector =
            ModelSelector::from_model_entry(&model_entry_with_reasoning(), None, None);
        // Move to non-reasoning model (DeepSeek)
        selector.on_event(UiEvent::Key(key(KeyCode::Down)));
        assert!(!selector.combobox.selected().unwrap().supports_reasoning);

        selector.on_event(UiEvent::Key(key(KeyCode::Right)));
        assert_eq!(selector.reasoning_effort, None);
    }

    #[test]
    fn render_shows_bar_on_focused_reasoning_row() {
        use crate::tui::test_picker::rendered_raw_lines;
        let mut selector =
            ModelSelector::from_model_entry(&model_entry_with_reasoning(), None, Some("medium"));
        let lines = rendered_raw_lines(&mut selector);
        let focused_line = lines
            .iter()
            .find(|l| l.plain_text().contains("▶"))
            .expect("should have focused line");
        let text = focused_line.plain_text();
        // Medium = 2 filled, 1 empty
        assert!(text.contains("▰▰▱"), "expected reasoning bar, got: {text}");
    }

    #[test]
    fn render_no_bar_on_non_reasoning_focused_row() {
        let mut selector =
            ModelSelector::from_model_entry(&model_entry_with_reasoning(), None, Some("medium"));
        // Move to non-reasoning model
        selector.on_event(UiEvent::Key(key(KeyCode::Down)));
        let lines = rendered_lines(&mut selector);
        let focused_line = lines
            .iter()
            .find(|l| l.contains("▶"))
            .expect("should have focused line");
        assert!(
            !focused_line.contains('▰'),
            "should not show bar on non-reasoning model"
        );
    }

    #[test]
    fn confirm_returns_both_model_and_reasoning_changes() {
        let mut selector =
            ModelSelector::from_model_entry(&model_entry_with_reasoning(), None, None);
        // Toggle a model on
        selector.on_event(UiEvent::Key(space()));
        // Change reasoning
        selector.on_event(UiEvent::Key(key(KeyCode::Right)));

        let changes = selector.confirm();
        assert_eq!(changes.len(), 2, "expected model + reasoning changes");
        assert!(changes.iter().any(|c| c.config_id == "model"));
        assert!(
            changes
                .iter()
                .any(|c| c.config_id == "reasoning_effort" && c.new_value == "low")
        );
    }

    #[test]
    fn confirm_returns_only_reasoning_when_only_reasoning_changed() {
        let mut selector = ModelSelector::from_model_entry(
            &model_entry_with_reasoning(),
            Some("anthropic:claude-opus-4-6"),
            None,
        );
        // Don't change models, just reasoning
        selector.on_event(UiEvent::Key(key(KeyCode::Right)));
        selector.on_event(UiEvent::Key(key(KeyCode::Right)));

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
