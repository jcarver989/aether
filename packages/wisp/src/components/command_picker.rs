use crate::tui::{Combobox, Searchable};
use crate::tui::{Component, HandlesInput, InputOutcome, Line, RenderContext};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

pub enum CommandPickerAction {
    CloseAndClearInput,
    CommandChosen(CommandEntry),
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
        lines.push(Line::styled(header, context.theme.muted));

        if self.combobox.matches.is_empty() {
            lines.push(Line::new("  (no matching commands)".to_string()));
            return lines;
        }

        for (i, command) in self.combobox.visible_matches().iter().enumerate() {
            let prefix = if Some(i) == self.combobox.visible_selected_index() {
                "▶ "
            } else {
                "  "
            };

            let hint_suffix = match &command.hint {
                Some(hint) => format!("  [{hint}]"),
                None => String::new(),
            };

            let line_text = format!(
                "{prefix}/{} - {}{}",
                command.name, command.description, hint_suffix
            );
            let line = if Some(i) == self.combobox.visible_selected_index() {
                Line::styled(line_text, context.theme.primary)
            } else {
                let name_part = format!("{prefix}/{}", command.name);
                let desc_part = format!(" - {}", command.description);
                let hint_part = hint_suffix;
                let mut line = Line::new(name_part);
                line.push_styled(desc_part, context.theme.muted);
                line.push_styled(hint_part, context.theme.muted);
                line
            };
            lines.push(line);
        }

        lines
    }
}

impl HandlesInput for CommandPicker {
    type Action = CommandPickerAction;

    fn handle_key(
        &mut self,
        key_event: KeyEvent,
        input: &mut String,
    ) -> InputOutcome<Self::Action> {
        match key_event.code {
            KeyCode::Esc => {
                input.clear();
                InputOutcome::action_and_render(CommandPickerAction::CloseAndClearInput)
            }
            KeyCode::Up => {
                self.combobox.move_selection_up();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Char('p') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.combobox.move_selection_up();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Down => {
                self.combobox.move_selection_down();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Char('n') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.combobox.move_selection_down();
                InputOutcome::consumed_and_render()
            }
            KeyCode::Enter => {
                if let Some(command) = self.selected_command().cloned() {
                    InputOutcome::action(CommandPickerAction::CommandChosen(command))
                } else {
                    input.clear();
                    InputOutcome::action_and_render(CommandPickerAction::CloseAndClearInput)
                }
            }
            KeyCode::Char(c) => {
                if c.is_control() {
                    return InputOutcome::consumed();
                }
                self.combobox.push_query_char(c);
                InputOutcome::consumed_and_render()
            }
            KeyCode::Backspace => {
                if self.combobox.query.is_empty() {
                    input.clear();
                    InputOutcome::action_and_render(CommandPickerAction::CloseAndClearInput)
                } else {
                    self.combobox.pop_query_char();
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
        let text: Vec<String> = lines.iter().map(|l| l.plain_text()).collect();

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
            .find(|l| l.plain_text().contains("/config"))
            .expect("config command should be rendered");
        assert!(
            !config_line.plain_text().contains("  ["),
            "Config command should not have hint brackets. Got: {}",
            config_line.plain_text()
        );
    }

    #[test]
    fn handle_key_enter_returns_selected_command() {
        let mut picker = CommandPicker::new(sample_commands());
        let mut input = "/".to_string();

        let outcome = picker.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut input,
        );

        assert!(outcome.consumed);
        assert!(matches!(
            outcome.action,
            Some(CommandPickerAction::CommandChosen(_))
        ));
    }

    #[test]
    fn handle_key_backspace_on_empty_query_requests_close() {
        let mut picker = CommandPicker::new(sample_commands());
        let mut input = "/".to_string();
        picker.combobox.query.clear();

        let outcome = picker.handle_key(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &mut input,
        );

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(
            outcome.action,
            Some(CommandPickerAction::CloseAndClearInput)
        ));
        assert_eq!(input, "");
    }
}
