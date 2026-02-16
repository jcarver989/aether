use crate::components::config_menu::{ConfigChange, ConfigMenuEntry, ConfigMenuValue};
use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};
use std::sync::Arc;

const MAX_VISIBLE_MATCHES: u32 = 10;
const MATCH_TIMEOUT_MS: u64 = 10;
const MAX_TICKS_PER_QUERY: usize = 4;

#[derive(Debug, Clone)]
struct IndexedOption {
    value: String,
    name: String,
    description: Option<String>,
    is_disabled: bool,
    search_text: String,
}

struct ConfigOptionSearchEngine {
    matcher: Nucleo<IndexedOption>,
}

pub struct ConfigPicker {
    pub config_id: String,
    pub title: String,
    pub query: String,
    pub matches: Vec<ConfigMenuValue>,
    pub selected_index: usize,
    current_value: String,
    search_engine: ConfigOptionSearchEngine,
}

pub struct ConfigPickerComponent<'a> {
    pub picker: &'a ConfigPicker,
}

impl ConfigPicker {
    pub fn from_entry(entry: &ConfigMenuEntry) -> Option<Self> {
        if entry.values.is_empty() {
            return None;
        }

        let current_value = entry.values.get(entry.current_value_index)?.value.clone();
        let mut picker = Self {
            config_id: entry.config_id.clone(),
            title: entry.title.clone(),
            query: String::new(),
            matches: Vec::new(),
            selected_index: 0,
            current_value,
            search_engine: ConfigOptionSearchEngine::from_values(entry.values.clone()),
        };
        picker.matches = picker.search_engine.search("", false);
        picker.selected_index = picker
            .matches
            .iter()
            .position(|m| m.value == picker.current_value)
            .unwrap_or(0);
        picker.ensure_selectable();
        Some(picker)
    }

    pub fn update_query(&mut self, query: String) {
        let append = query.starts_with(&self.query);
        self.query = query;
        self.matches = self.search_engine.search(&self.query, append);
        if self.selected_index >= self.matches.len() {
            self.selected_index = 0;
        }
        self.ensure_selectable();
    }

    pub fn push_query_char(&mut self, c: char) {
        let mut query = self.query.clone();
        query.push(c);
        self.update_query(query);
    }

    pub fn pop_query_char(&mut self) {
        if self.query.is_empty() {
            return;
        }
        let mut query = self.query.clone();
        query.pop();
        self.update_query(query);
    }

    pub fn move_selection_up(&mut self) {
        if self.matches.is_empty() {
            return;
        }

        let len = self.matches.len();
        let mut idx = self.selected_index;
        for _ in 0..len {
            if idx > 0 {
                idx -= 1;
            } else {
                idx = len - 1;
            }
            if !self.matches[idx].is_disabled {
                self.selected_index = idx;
                return;
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        if self.matches.is_empty() {
            return;
        }

        let len = self.matches.len();
        let mut idx = self.selected_index;
        for _ in 0..len {
            idx = (idx + 1) % len;
            if !self.matches[idx].is_disabled {
                self.selected_index = idx;
                return;
            }
        }
    }

    pub fn confirm_selection(&self) -> Option<ConfigChange> {
        let selected = self.matches.get(self.selected_index)?;
        if selected.is_disabled || selected.value == self.current_value {
            return None;
        }

        Some(ConfigChange {
            config_id: self.config_id.clone(),
            new_value: selected.value.clone(),
        })
    }

    fn first_enabled_index(&self) -> Option<usize> {
        self.matches.iter().position(|m| !m.is_disabled)
    }

    fn ensure_selectable(&mut self) {
        if self.matches.is_empty() {
            self.selected_index = 0;
            return;
        }
        if self.selected_index >= self.matches.len()
            || self.matches[self.selected_index].is_disabled
        {
            self.selected_index = self.first_enabled_index().unwrap_or(0);
        }
    }
}

impl ConfigOptionSearchEngine {
    fn from_values(values: Vec<ConfigMenuValue>) -> Self {
        let entries: Vec<IndexedOption> = values
            .into_iter()
            .map(|value| {
                let search_text = format!("{} {}", value.name, value.value);
                IndexedOption {
                    value: value.value,
                    name: value.name,
                    description: value.description,
                    is_disabled: value.is_disabled,
                    search_text,
                }
            })
            .collect();

        let mut matcher = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), Some(1), 1);
        let injector = matcher.injector();
        for entry in entries {
            injector.push(entry, |item, columns| {
                columns[0] = item.search_text.as_str().into();
            });
        }
        let _ = matcher.tick(0);
        Self { matcher }
    }

    fn search(&mut self, query: &str, append: bool) -> Vec<ConfigMenuValue> {
        self.matcher
            .pattern
            .reparse(0, query, CaseMatching::Smart, Normalization::Smart, append);
        let mut status = self.matcher.tick(MATCH_TIMEOUT_MS);
        let mut ticks = 0;
        while status.running && ticks < MAX_TICKS_PER_QUERY {
            status = self.matcher.tick(MATCH_TIMEOUT_MS);
            ticks += 1;
        }

        let snapshot = self.matcher.snapshot();
        let limit = snapshot.matched_item_count().min(MAX_VISIBLE_MATCHES);
        snapshot
            .matched_items(0..limit)
            .map(|item| ConfigMenuValue {
                value: item.data.value.clone(),
                name: item.data.name.clone(),
                description: item.data.description.clone(),
                is_disabled: item.data.is_disabled,
            })
            .collect()
    }
}

impl Component for ConfigPickerComponent<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        let header = format!("  {} search: {}", self.picker.title, self.picker.query);
        lines.push(Line::new(header.with(context.theme.muted).to_string()));

        if self.picker.matches.is_empty() {
            lines.push(Line::new("  (no matches found)".to_string()));
            return lines;
        }

        for (i, option) in self.picker.matches.iter().enumerate() {
            let prefix = if i == self.picker.selected_index {
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
            } else if i == self.picker.selected_index {
                Line::new(line_text.with(context.theme.primary).to_string())
            } else {
                Line::new(line_text)
            };
            lines.push(line);
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(picker.selected_index, 0);
        assert_eq!(picker.matches[picker.selected_index].name, "GPT-4o");
    }

    #[test]
    fn query_filters_by_name_or_value() {
        let mut picker = ConfigPicker::from_entry(&entry()).expect("picker");
        picker.update_query("gemini".to_string());
        assert_eq!(picker.matches.len(), 1);
        assert_eq!(picker.matches[0].name, "Gemini 2.5 Pro");

        picker.update_query("anthropic/claude".to_string());
        assert_eq!(picker.matches.len(), 1);
        assert_eq!(picker.matches[0].name, "Claude Sonnet");
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
}
