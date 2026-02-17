use crate::components::combobox::{Combobox, Searchable};
use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;

#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
    pub has_input: bool,
    pub hint: Option<String>,
    pub builtin: bool,
}

impl Searchable for CommandEntry {
    fn search_text(&self) -> String {
        format!("{} {}", self.name, self.description)
    }
}

pub struct CommandPicker {
    pub combobox: Combobox<CommandEntry>,
}

impl CommandPicker {
    pub fn new(commands: Vec<CommandEntry>) -> Self {
        Self {
            combobox: Combobox::new(commands),
        }
    }

    pub fn selected_command(&self) -> Option<&CommandEntry> {
        self.combobox.selected()
    }
}

impl Component for CommandPicker {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        let header = format!("  / search: {}", self.combobox.query);
        lines.push(Line::new(header.with(context.theme.muted).to_string()));

        if self.combobox.matches.is_empty() {
            lines.push(Line::new("  (no matching commands)".to_string()));
            return lines;
        }

        for (i, command) in self.combobox.matches.iter().enumerate() {
            let prefix = if i == self.combobox.selected_index {
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
            let line = if i == self.combobox.selected_index {
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
        assert_eq!(picker.combobox.matches.len(), 3);
    }

    #[test]
    fn query_filters_by_name() {
        let mut picker = CommandPicker::new(sample_commands());
        picker.combobox.update_query("conf".to_string());
        assert_eq!(picker.combobox.matches.len(), 1);
        assert_eq!(picker.combobox.matches[0].name, "config");
    }

    #[test]
    fn query_filters_by_description() {
        let mut picker = CommandPicker::new(sample_commands());
        picker.combobox.update_query("browse".to_string());
        assert_eq!(picker.combobox.matches.len(), 1);
        assert_eq!(picker.combobox.matches[0].name, "web");
    }

    #[test]
    fn selection_wraps() {
        let mut picker = CommandPicker::new(sample_commands());

        picker.combobox.move_selection_up();
        assert_eq!(picker.combobox.selected_index, 2);

        picker.combobox.move_selection_down();
        assert_eq!(picker.combobox.selected_index, 0);
    }

    #[test]
    fn selected_command_returns_correct_entry() {
        let mut picker = CommandPicker::new(sample_commands());
        let first = picker.selected_command().unwrap().name.clone();
        picker.combobox.move_selection_down();
        let second = picker.selected_command().unwrap().name.clone();
        assert_ne!(first, second);
    }

    #[test]
    fn push_and_pop_query_char() {
        let mut picker = CommandPicker::new(sample_commands());
        picker.combobox.push_query_char('c');
        picker.combobox.push_query_char('o');
        assert_eq!(picker.combobox.query, "co");

        picker.combobox.pop_query_char();
        assert_eq!(picker.combobox.query, "c");

        picker.combobox.pop_query_char();
        assert_eq!(picker.combobox.query, "");

        // pop on empty is a no-op
        picker.combobox.pop_query_char();
        assert_eq!(picker.combobox.query, "");
    }

    #[test]
    fn render_includes_hint_for_commands_with_hint() {
        let picker = CommandPicker::new(sample_commands());
        let context = RenderContext::new((120, 40));
        let lines = picker.render(&context);
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
        picker.combobox.selected_index = 1;
        let context = RenderContext::new((120, 40));
        let lines = picker.render(&context);

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
