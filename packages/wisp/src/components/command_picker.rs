use crate::tui::{
    Combobox, Component, HandlesInput, InputOutcome, Line, PickerKey, RenderContext, Searchable,
    classify_key,
};
use crossterm::event::KeyEvent;

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
    combobox: Combobox<CommandEntry>,
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

    pub fn query(&self) -> &str {
        self.combobox.query()
    }

    pub fn matches(&self) -> &[CommandEntry] {
        self.combobox.matches()
    }
}

impl Component for CommandPicker {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();
        let header = format!("  / search: {}", self.combobox.query());
        lines.push(Line::styled(header, context.theme.muted));

        if self.combobox.is_empty() {
            lines.push(Line::new("  (no matching commands)".to_string()));
            return lines;
        }

        let item_lines = self
            .combobox
            .render_items(context, |command, is_selected, ctx| {
                let prefix = if is_selected { "▶ " } else { "  " };

                let hint_suffix = match &command.hint {
                    Some(hint) => format!("  [{hint}]"),
                    None => String::new(),
                };

                let line_text = format!(
                    "{prefix}/{} - {}{}",
                    command.name, command.description, hint_suffix
                );
                if is_selected {
                    Line::styled(line_text, ctx.theme.primary)
                } else {
                    let name_part = format!("{prefix}/{}", command.name);
                    let desc_part = format!(" - {}", command.description);
                    let hint_part = hint_suffix;
                    let mut line = Line::new(name_part);
                    line.push_styled(desc_part, ctx.theme.muted);
                    line.push_styled(hint_part, ctx.theme.muted);
                    line
                }
            });
        lines.extend(item_lines);

        lines
    }
}

impl HandlesInput for CommandPicker {
    type Action = CommandPickerAction;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action> {
        match classify_key(key_event, self.combobox.query().is_empty()) {
            PickerKey::Escape | PickerKey::BackspaceOnEmpty => {
                InputOutcome::action_and_render(CommandPickerAction::CloseAndClearInput)
            }
            PickerKey::MoveUp => {
                self.combobox.move_up();
                InputOutcome::consumed_and_render()
            }
            PickerKey::MoveDown => {
                self.combobox.move_down();
                InputOutcome::consumed_and_render()
            }
            PickerKey::Confirm => {
                if let Some(command) = self.combobox.selected().cloned() {
                    InputOutcome::action(CommandPickerAction::CommandChosen(command))
                } else {
                    InputOutcome::action_and_render(CommandPickerAction::CloseAndClearInput)
                }
            }
            PickerKey::Char(c) => {
                self.combobox.push_query_char(c);
                InputOutcome::consumed_and_render()
            }
            PickerKey::Backspace => {
                self.combobox.pop_query_char();
                InputOutcome::consumed_and_render()
            }
            PickerKey::ControlChar | PickerKey::Other => InputOutcome::consumed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_picker::{rendered_lines, selected_text, type_query};
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
        let mut picker = CommandPicker::new(sample_commands());
        let lines = rendered_lines(&mut picker);
        // header + 3 command lines
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().any(|l| l.contains("/config")));
        assert!(lines.iter().any(|l| l.contains("/search")));
        assert!(lines.iter().any(|l| l.contains("/web")));
    }

    #[test]
    fn query_filters_by_name() {
        let mut picker = CommandPicker::new(sample_commands());
        type_query(&mut picker, "conf");
        let lines = rendered_lines(&mut picker);
        // header + 1 match
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("/config"));
    }

    #[test]
    fn query_filters_by_description() {
        let mut picker = CommandPicker::new(sample_commands());
        type_query(&mut picker, "browse");
        let lines = rendered_lines(&mut picker);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("/web"));
    }

    #[test]
    fn selection_wraps() {
        let mut picker = CommandPicker::new(sample_commands());
        let first = selected_text(&mut picker).unwrap();

        picker.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let last = selected_text(&mut picker).unwrap();
        assert_ne!(first, last);

        picker.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let back_to_first = selected_text(&mut picker).unwrap();
        assert_eq!(first, back_to_first);
    }

    #[test]
    fn selected_command_changes_on_move() {
        let mut picker = CommandPicker::new(sample_commands());
        let first = selected_text(&mut picker).unwrap();
        picker.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let second = selected_text(&mut picker).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn type_and_delete_updates_query() {
        let mut picker = CommandPicker::new(sample_commands());
        type_query(&mut picker, "co");
        assert_eq!(picker.query(), "co");

        picker.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(picker.query(), "c");

        picker.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(picker.query(), "");
    }

    #[test]
    fn render_includes_hint_for_commands_with_hint() {
        let mut picker = CommandPicker::new(sample_commands());
        let lines = rendered_lines(&mut picker);

        assert!(
            lines.iter().any(|l| l.contains("[query pattern]")),
            "Should render hint for search command. Got: {lines:?}",
        );
        assert!(
            lines.iter().any(|l| l.contains("[url]")),
            "Should render hint for web command. Got: {lines:?}",
        );
    }

    #[test]
    fn render_omits_hint_brackets_for_commands_without_hint() {
        let mut picker = CommandPicker::new(sample_commands());
        let lines = rendered_lines(&mut picker);

        let config_line = lines
            .iter()
            .find(|l| l.contains("/config"))
            .expect("config command should be rendered");
        assert!(
            !config_line.contains("  ["),
            "Config command should not have hint brackets. Got: {config_line}",
        );
    }

    #[test]
    fn handle_key_enter_returns_selected_command() {
        let mut picker = CommandPicker::new(sample_commands());

        let outcome = picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(outcome.consumed);
        assert!(matches!(
            outcome.action,
            Some(CommandPickerAction::CommandChosen(_))
        ));
    }

    #[test]
    fn handle_key_backspace_on_empty_query_requests_close() {
        let mut picker = CommandPicker::new(sample_commands());

        let outcome = picker.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(
            outcome.action,
            Some(CommandPickerAction::CloseAndClearInput)
        ));
    }
}
