use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};
use std::sync::Arc;

const MAX_VISIBLE_MATCHES: u32 = 10;
const MATCH_TIMEOUT_MS: u64 = 10;
const MAX_TICKS_PER_QUERY: usize = 4;

#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
    pub has_input: bool,
    pub hint: Option<String>,
    pub builtin: bool,
}

pub struct CommandPicker {
    pub query: String,
    pub matches: Vec<CommandEntry>,
    pub selected_index: usize,
    search_engine: CommandSearchEngine,
}

pub struct CommandPickerComponent<'a> {
    pub picker: &'a CommandPicker,
}

#[derive(Debug, Clone)]
struct IndexedCommand {
    name: String,
    description: String,
    has_input: bool,
    hint: Option<String>,
    builtin: bool,
    search_text: String,
}

struct CommandSearchEngine {
    matcher: Nucleo<IndexedCommand>,
}

impl CommandPicker {
    pub fn new(commands: Vec<CommandEntry>) -> Self {
        let mut picker = Self {
            query: String::new(),
            matches: Vec::new(),
            selected_index: 0,
            search_engine: CommandSearchEngine::from_entries(commands),
        };
        picker.matches = picker.search_engine.search("", false);
        picker
    }

    pub fn update_query(&mut self, query: String) {
        let append = query.starts_with(&self.query);
        self.query = query;
        self.matches = self.search_engine.search(&self.query, append);
        if self.selected_index >= self.matches.len() {
            self.selected_index = 0;
        }
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
        if !self.matches.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.matches.len() - 1;
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        if !self.matches.is_empty() {
            if self.selected_index < self.matches.len() - 1 {
                self.selected_index += 1;
            } else {
                self.selected_index = 0;
            }
        }
    }

    pub fn selected_command(&self) -> Option<&CommandEntry> {
        self.matches.get(self.selected_index)
    }
}

impl CommandSearchEngine {
    fn from_entries(entries: Vec<CommandEntry>) -> Self {
        let indexed: Vec<IndexedCommand> = entries
            .into_iter()
            .map(|entry| {
                let search_text = format!("{} {}", entry.name, entry.description);
                IndexedCommand {
                    name: entry.name,
                    description: entry.description,
                    has_input: entry.has_input,
                    hint: entry.hint,
                    builtin: entry.builtin,
                    search_text,
                }
            })
            .collect();

        let mut matcher = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), Some(1), 1);
        let injector = matcher.injector();
        for entry in indexed {
            injector.push(entry, |item, columns| {
                columns[0] = item.search_text.as_str().into();
            });
        }
        let _ = matcher.tick(0);
        Self { matcher }
    }

    fn search(&mut self, query: &str, append: bool) -> Vec<CommandEntry> {
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
            .map(|item| CommandEntry {
                name: item.data.name.clone(),
                description: item.data.description.clone(),
                has_input: item.data.has_input,
                hint: item.data.hint.clone(),
                builtin: item.data.builtin,
            })
            .collect()
    }
}

impl Component for CommandPickerComponent<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        let header = format!("  / search: {}", self.picker.query);
        lines.push(Line::new(header.with(context.theme.muted).to_string()));

        if self.picker.matches.is_empty() {
            lines.push(Line::new("  (no matching commands)".to_string()));
            return lines;
        }

        for (i, command) in self.picker.matches.iter().enumerate() {
            let prefix = if i == self.picker.selected_index {
                "▶ "
            } else {
                "  "
            };

            let hint_suffix = match &command.hint {
                Some(hint) => format!("  [{}]", hint),
                None => String::new(),
            };

            let line_text = format!(
                "{prefix}/{} - {}{}",
                command.name, command.description, hint_suffix
            );
            let line = if i == self.picker.selected_index {
                Line::new(line_text.with(context.theme.primary).to_string())
            } else {
                let name_part = format!("{prefix}/{}", command.name);
                let desc_part = format!(" - {}", command.description);
                let hint_part = hint_suffix;
                Line::new(format!(
                    "{}{}{}",
                    name_part,
                    desc_part.with(context.theme.muted),
                    hint_part.with(context.theme.muted),
                ))
            };
            lines.push(line);
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_commands() -> Vec<CommandEntry> {
        vec![
            CommandEntry {
                name: "config".into(),
                description: "Open configuration settings".into(),
                has_input: false,
                hint: None,
                builtin: true,
            },
            CommandEntry {
                name: "search".into(),
                description: "Search code in the project".into(),
                has_input: true,
                hint: Some("query pattern".into()),
                builtin: false,
            },
            CommandEntry {
                name: "web".into(),
                description: "Browse the web".into(),
                has_input: true,
                hint: Some("url".into()),
                builtin: false,
            },
        ]
    }

    #[test]
    fn init_shows_all_commands() {
        let picker = CommandPicker::new(sample_commands());
        assert_eq!(picker.matches.len(), 3);
    }

    #[test]
    fn query_filters_by_name() {
        let mut picker = CommandPicker::new(sample_commands());
        picker.update_query("conf".to_string());
        assert_eq!(picker.matches.len(), 1);
        assert_eq!(picker.matches[0].name, "config");
    }

    #[test]
    fn query_filters_by_description() {
        let mut picker = CommandPicker::new(sample_commands());
        picker.update_query("browse".to_string());
        assert_eq!(picker.matches.len(), 1);
        assert_eq!(picker.matches[0].name, "web");
    }

    #[test]
    fn selection_wraps() {
        let mut picker = CommandPicker::new(sample_commands());

        picker.move_selection_up();
        assert_eq!(picker.selected_index, 2);

        picker.move_selection_down();
        assert_eq!(picker.selected_index, 0);
    }

    #[test]
    fn selected_command_returns_correct_entry() {
        let mut picker = CommandPicker::new(sample_commands());
        let first = picker.selected_command().unwrap().name.clone();
        picker.move_selection_down();
        let second = picker.selected_command().unwrap().name.clone();
        assert_ne!(first, second);
    }

    #[test]
    fn push_and_pop_query_char() {
        let mut picker = CommandPicker::new(sample_commands());
        picker.push_query_char('c');
        picker.push_query_char('o');
        assert_eq!(picker.query, "co");

        picker.pop_query_char();
        assert_eq!(picker.query, "c");

        picker.pop_query_char();
        assert_eq!(picker.query, "");

        // pop on empty is a no-op
        picker.pop_query_char();
        assert_eq!(picker.query, "");
    }

    #[test]
    fn render_includes_hint_for_commands_with_hint() {
        let picker = CommandPicker::new(sample_commands());
        let component = CommandPickerComponent { picker: &picker };
        let context = RenderContext::new((120, 40));
        let lines = component.render(&context);
        let text: Vec<&str> = lines.iter().map(|l| l.as_str()).collect();

        assert!(
            text.iter().any(|l| l.contains("[query pattern]")),
            "Should render hint for search command. Got: {:?}",
            text
        );
        assert!(
            text.iter().any(|l| l.contains("[url]")),
            "Should render hint for web command. Got: {:?}",
            text
        );
    }

    #[test]
    fn render_omits_hint_brackets_for_commands_without_hint() {
        let mut picker = CommandPicker::new(sample_commands());
        // Move selection away from config so it renders without ANSI highlight
        picker.selected_index = 1;
        let component = CommandPickerComponent { picker: &picker };
        let context = RenderContext::new((120, 40));
        let lines = component.render(&context);

        let config_line = lines
            .iter()
            .find(|l| l.as_str().contains("/config"))
            .expect("config command should be rendered");
        assert!(
            !config_line.as_str().contains("  ["),
            "Config command should not have hint brackets. Got: {}",
            config_line.as_str()
        );
    }
}
