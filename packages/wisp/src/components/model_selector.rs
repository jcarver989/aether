use super::reasoning_bar::reasoning_bar;
use crate::settings::types::SettingsChange;
use std::cmp::Ordering;
use std::collections::HashSet;
use tui::{
    Combobox, Component, Event, Frame, Line, MouseEventKind, PickerKey, Searchable, ViewContext,
    classify_key,
};
use utils::ReasoningEffort;

#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub value: String,
    pub name: String,
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

const REASONING_EFFORT_CONFIG_ID: &str = "reasoning_effort";

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
    pub fn new(
        items: Vec<ModelEntry>,
        config_id: String,
        current_selection: Option<&str>,
        current_reasoning_effort: Option<&str>,
    ) -> Self {
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
            config_id,
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
                config_id: REASONING_EFFORT_CONFIG_ID.to_string(),
                new_value: ReasoningEffort::config_str(self.reasoning_effort).to_string(),
            });
        }
        changes
    }
}

impl ModelSelector {
    pub fn update_viewport(&mut self, max_height: usize) {
        let header_lines = if self.selected_models.is_empty() {
            2
        } else {
            4
        };
        let available = max_height.saturating_sub(header_lines);

        let mut max_items = available;
        for _ in 0..3 {
            self.combobox.set_max_visible(max_items.max(1));
            let matches = self.combobox.visible_matches_with_selection();
            let groups = count_provider_groups(&matches);
            let interstitial = if groups > 0 {
                groups + groups.saturating_sub(1)
            } else {
                0
            };
            let needed = max_items + interstitial;
            if needed <= available {
                break;
            }
            max_items = available.saturating_sub(interstitial);
        }
        self.combobox.set_max_visible(max_items.max(1));
    }
}

impl Component for ModelSelector {
    type Message = ModelSelectorMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        if let Event::Mouse(mouse) = event {
            return match mouse.kind {
                MouseEventKind::ScrollUp => {
                    self.combobox.move_up();
                    Some(vec![])
                }
                MouseEventKind::ScrollDown => {
                    self.combobox.move_down();
                    Some(vec![])
                }
                _ => Some(vec![]),
            };
        }
        let Event::Key(key) = event else {
            return None;
        };
        match classify_key(*key, self.combobox.query().is_empty()) {
            PickerKey::Escape => {
                let changes = self.confirm();
                Some(vec![ModelSelectorMessage::Done(changes)])
            }
            PickerKey::MoveUp => {
                self.combobox.move_up();
                Some(vec![])
            }
            PickerKey::MoveDown => {
                self.combobox.move_down();
                Some(vec![])
            }
            PickerKey::Tab => {
                if self
                    .combobox
                    .selected()
                    .is_some_and(|e| e.supports_reasoning)
                {
                    self.reasoning_effort = cycle_reasoning(self.reasoning_effort);
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
            PickerKey::MoveLeft
            | PickerKey::MoveRight
            | PickerKey::BackTab
            | PickerKey::BackspaceOnEmpty
            | PickerKey::ControlChar
            | PickerKey::Other => Some(vec![]),
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

                if *is_focused {
                    let mut line = Line::with_style(label, context.theme.selected_row_style());
                    if entry.supports_reasoning {
                        let bar = reasoning_bar(self.reasoning_effort);
                        line.push_with_style(
                            format!("    {bar}"),
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

fn count_provider_groups(items: &[(&ModelEntry, bool)]) -> usize {
    let mut count = 0;
    let mut last_provider: Option<&str> = None;
    for (entry, _) in items {
        let provider = entry.provider_key();
        if last_provider != Some(provider) {
            count += 1;
            last_provider = Some(provider);
        }
    }
    count
}

fn cycle_reasoning(effort: Option<ReasoningEffort>) -> Option<ReasoningEffort> {
    match effort {
        None => Some(ReasoningEffort::Low),
        Some(ReasoningEffort::Low) => Some(ReasoningEffort::Medium),
        Some(ReasoningEffort::Medium) => Some(ReasoningEffort::High),
        Some(ReasoningEffort::High) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tui::{KeyCode, KeyEvent, KeyModifiers};

    fn make_items() -> Vec<ModelEntry> {
        vec![
            ModelEntry {
                value: "anthropic:claude-sonnet-4-5".to_string(),
                name: "Anthropic / Claude Sonnet 4.5".to_string(),

                supports_reasoning: false,
            },
            ModelEntry {
                value: "deepseek:deepseek-chat".to_string(),
                name: "DeepSeek / DeepSeek Chat".to_string(),

                supports_reasoning: false,
            },
            ModelEntry {
                value: "gemini:gemini-2.5-pro".to_string(),
                name: "Google / Gemini 2.5 Pro".to_string(),

                supports_reasoning: false,
            },
        ]
    }

    fn make_selector() -> ModelSelector {
        ModelSelector::new(make_items(), "model".to_string(), None, None)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn space() -> KeyEvent {
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)
    }

    #[tokio::test]
    async fn toggle_adds_and_removes_model() {
        let mut s = make_selector();
        assert_eq!(s.selected_count(), 0);

        s.on_event(&Event::Key(space())).await;
        assert_eq!(s.selected_count(), 1);

        s.on_event(&Event::Key(space())).await;
        assert_eq!(s.selected_count(), 0);
    }

    #[test]
    fn confirm_with_zero_returns_empty() {
        let s = make_selector();
        assert!(s.confirm().is_empty());
    }

    #[tokio::test]
    async fn confirm_with_one_returns_single_model() {
        let mut s = make_selector();
        s.on_event(&Event::Key(space())).await;
        let changes = s.confirm();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].config_id, "model");
        assert_eq!(changes[0].new_value, "anthropic:claude-sonnet-4-5");
    }

    #[tokio::test]
    async fn confirm_with_two_returns_comma_joined() {
        let mut s = make_selector();
        s.on_event(&Event::Key(space())).await;
        s.on_event(&Event::Key(key(KeyCode::Down))).await;
        s.on_event(&Event::Key(space())).await;

        let changes = s.confirm();
        assert_eq!(changes.len(), 1);
        let parts: HashSet<&str> = changes[0].new_value.split(',').collect();
        assert!(parts.contains("anthropic:claude-sonnet-4-5"));
        assert!(parts.contains("deepseek:deepseek-chat"));
    }

    #[test]
    fn pre_selected_values_from_current_selection() {
        let s = ModelSelector::new(
            make_items(),
            "model".to_string(),
            Some("anthropic:claude-sonnet-4-5,deepseek:deepseek-chat"),
            None,
        );
        assert_eq!(s.selected_count(), 2);
    }

    #[tokio::test]
    async fn escape_returns_done_action() {
        let mut s = make_selector();
        let outcome = s.on_event(&Event::Key(key(KeyCode::Esc))).await;
        let messages = outcome.unwrap();
        match messages.as_slice() {
            [ModelSelectorMessage::Done(changes)] => assert!(changes.is_empty()),
            other => panic!("expected Done([]), got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn enter_toggles_focused_model() {
        let mut s = make_selector();
        assert_eq!(s.selected_count(), 0);

        s.on_event(&Event::Key(key(KeyCode::Enter))).await;
        assert_eq!(s.selected_count(), 1);

        s.on_event(&Event::Key(key(KeyCode::Enter))).await;
        assert_eq!(s.selected_count(), 0);
    }

    #[tokio::test]
    async fn escape_with_selections_returns_done_with_change() {
        let mut s = make_selector();
        s.on_event(&Event::Key(space())).await;
        s.on_event(&Event::Key(key(KeyCode::Down))).await;
        s.on_event(&Event::Key(space())).await;

        let outcome = s.on_event(&Event::Key(key(KeyCode::Esc))).await;
        let messages = outcome.unwrap();
        match messages.as_slice() {
            [ModelSelectorMessage::Done(changes)] => {
                assert_eq!(changes.len(), 1);
                let parts: HashSet<&str> = changes[0].new_value.split(',').collect();
                assert!(parts.contains("anthropic:claude-sonnet-4-5"));
                assert!(parts.contains("deepseek:deepseek-chat"));
            }
            other => panic!("expected Done with model change, got: {other:?}"),
        }
    }

    #[test]
    fn escape_without_toggle_returns_no_change() {
        let s = ModelSelector::new(
            make_items(),
            "model".to_string(),
            Some("anthropic:claude-sonnet-4-5,deepseek:deepseek-chat"),
            None,
        );
        assert!(s.confirm().is_empty());
    }

    #[tokio::test]
    async fn escape_after_toggle_returns_change() {
        let mut s = ModelSelector::new(
            make_items(),
            "model".to_string(),
            Some("anthropic:claude-sonnet-4-5"),
            None,
        );
        s.on_event(&Event::Key(key(KeyCode::Down))).await;
        s.on_event(&Event::Key(space())).await;
        let changes = s.confirm();
        assert_eq!(changes.len(), 1);
        let parts: HashSet<&str> = changes[0].new_value.split(',').collect();
        assert!(parts.contains("anthropic:claude-sonnet-4-5"));
        assert!(parts.contains("deepseek:deepseek-chat"));
    }

    fn make_reasoning_items() -> Vec<ModelEntry> {
        vec![
            ModelEntry {
                value: "anthropic:claude-opus-4-6".to_string(),
                name: "Anthropic / Claude Opus 4.6".to_string(),

                supports_reasoning: true,
            },
            ModelEntry {
                value: "deepseek:deepseek-chat".to_string(),
                name: "DeepSeek / DeepSeek Chat".to_string(),

                supports_reasoning: false,
            },
        ]
    }

    #[test]
    fn reasoning_cycle_wraps() {
        use ReasoningEffort::*;
        assert_eq!(cycle_reasoning(None), Some(Low));
        assert_eq!(cycle_reasoning(Some(Low)), Some(Medium));
        assert_eq!(cycle_reasoning(Some(Medium)), Some(High));
        assert_eq!(cycle_reasoning(Some(High)), None);
    }

    #[tokio::test]
    async fn tab_on_reasoning_model_cycles_level() {
        let mut s = ModelSelector::new(make_reasoning_items(), "model".to_string(), None, None);
        assert_eq!(s.reasoning_effort, None);

        s.on_event(&Event::Key(key(KeyCode::Tab))).await;
        assert_eq!(s.reasoning_effort, Some(ReasoningEffort::Low));

        s.on_event(&Event::Key(key(KeyCode::Tab))).await;
        assert_eq!(s.reasoning_effort, Some(ReasoningEffort::Medium));

        s.on_event(&Event::Key(key(KeyCode::Tab))).await;
        assert_eq!(s.reasoning_effort, Some(ReasoningEffort::High));

        s.on_event(&Event::Key(key(KeyCode::Tab))).await;
        assert_eq!(s.reasoning_effort, None);
    }

    #[tokio::test]
    async fn tab_on_non_reasoning_model_is_noop() {
        let mut s = ModelSelector::new(make_reasoning_items(), "model".to_string(), None, None);
        s.on_event(&Event::Key(key(KeyCode::Down))).await;
        assert!(!s.combobox.selected().unwrap().supports_reasoning);

        s.on_event(&Event::Key(key(KeyCode::Tab))).await;
        assert_eq!(s.reasoning_effort, None);
    }

    #[tokio::test]
    async fn confirm_returns_both_model_and_reasoning_changes() {
        let mut s = ModelSelector::new(make_reasoning_items(), "model".to_string(), None, None);
        s.on_event(&Event::Key(space())).await;
        s.on_event(&Event::Key(key(KeyCode::Tab))).await;

        let changes = s.confirm();
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
        let mut s = ModelSelector::new(
            make_reasoning_items(),
            "model".to_string(),
            Some("anthropic:claude-opus-4-6"),
            None,
        );
        s.on_event(&Event::Key(key(KeyCode::Tab))).await;
        s.on_event(&Event::Key(key(KeyCode::Tab))).await;

        let changes = s.confirm();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].config_id, "reasoning_effort");
        assert_eq!(changes[0].new_value, "medium");
    }

    #[test]
    fn confirm_returns_empty_when_nothing_changed() {
        let s = ModelSelector::new(
            make_reasoning_items(),
            "model".to_string(),
            Some("anthropic:claude-opus-4-6"),
            Some("high"),
        );
        assert!(s.confirm().is_empty());
    }

    #[tokio::test]
    async fn mouse_scroll_moves_selection() {
        use tui::{MouseEvent, MouseEventKind};

        let mut s = make_selector();
        let first = s.combobox.selected().unwrap().value.clone();

        let scroll_down = Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        let outcome = s.on_event(&scroll_down).await;
        assert!(outcome.is_some(), "mouse scroll should be consumed");

        let second = s.combobox.selected().unwrap().value.clone();
        assert_ne!(
            first, second,
            "scroll down should move to a different model"
        );

        let scroll_up = Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        s.on_event(&scroll_up).await;
        let back = s.combobox.selected().unwrap().value.clone();
        assert_eq!(first, back, "scroll up should return to the original model");
    }

    fn many_provider_items() -> Vec<ModelEntry> {
        ["a:m1", "b:m2", "c:m3", "d:m4", "e:m5", "f:m6"]
            .into_iter()
            .map(|v| {
                let (prov, model) = v.split_once(':').unwrap();
                ModelEntry {
                    value: v.to_string(),
                    name: format!("{} / {}", prov.to_uppercase(), model.to_uppercase()),
    
                    supports_reasoning: false,
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn focused_item_always_visible_after_scroll() {
        let mut s = ModelSelector::new(many_provider_items(), "model".to_string(), None, None);
        s.update_viewport(10);

        let ctx = ViewContext::new((80, 10));

        for _ in 0..6 {
            s.on_event(&Event::Key(key(KeyCode::Down))).await;
            let frame = s.render(&ctx);
            let lines = frame.lines();
            assert!(
                lines.iter().any(|l| l.plain_text().contains("▶")),
                "focused item (▶) must be visible after scrolling down, got: {:?}",
                lines.iter().map(|l| l.plain_text()).collect::<Vec<_>>()
            );
        }
    }
}
