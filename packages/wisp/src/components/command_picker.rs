use crate::tui::{
    Combobox, Component, Event, Line, PickerKey, PickerMessage, Searchable, Style, ViewContext,
    classify_key, display_width_text, pad_text_to_width, truncate_text,
};

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

pub type CommandPickerMessage = PickerMessage<CommandEntry>;

impl CommandPicker {
    pub fn new(commands: Vec<CommandEntry>) -> Self {
        Self {
            combobox: Combobox::new(commands),
        }
    }

    #[allow(dead_code)]
    pub fn query(&self) -> &str {
        self.combobox.query()
    }
}

impl Component for CommandPicker {
    type Message = CommandPickerMessage;

    fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(key_event) = event else {
            return None;
        };
        match classify_key(*key_event, self.combobox.query().is_empty()) {
            PickerKey::Escape => Some(vec![PickerMessage::Close]),
            PickerKey::BackspaceOnEmpty => Some(vec![PickerMessage::CloseAndPopChar]),
            PickerKey::MoveUp => {
                self.combobox.move_up();
                Some(vec![])
            }
            PickerKey::MoveDown => {
                self.combobox.move_down();
                Some(vec![])
            }
            PickerKey::Confirm => {
                if let Some(command) = self.combobox.selected().cloned() {
                    Some(vec![PickerMessage::Confirm(command)])
                } else {
                    Some(vec![PickerMessage::Close])
                }
            }
            PickerKey::Char(c) => {
                if c.is_whitespace() {
                    return Some(vec![PickerMessage::CloseWithChar(c)]);
                }
                self.combobox.push_query_char(c);
                Some(vec![PickerMessage::CharTyped(c)])
            }
            PickerKey::Backspace => {
                self.combobox.pop_query_char();
                Some(vec![PickerMessage::PopChar])
            }
            PickerKey::MoveLeft
            | PickerKey::MoveRight
            | PickerKey::ControlChar
            | PickerKey::Other => Some(vec![]),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
        let mut lines = Vec::new();

        if self.combobox.is_empty() {
            lines.push(Line::new("  (no matching commands)".to_string()));
            return lines;
        }

        let max_name_width = self
            .combobox
            .matches()
            .iter()
            .map(|cmd| display_width_text(&format!("  /{}", cmd.name)))
            .max()
            .unwrap_or(0);

        let item_lines = self
            .combobox
            .render_items(context, |command, is_selected, ctx| {
                let prefix = if is_selected { "▶ " } else { "  " };

                let hint_suffix = match &command.hint {
                    Some(hint) => format!("  [{hint}]"),
                    None => String::new(),
                };

                let name_part = format!("{prefix}/{}", command.name);
                let padded_name = pad_text_to_width(&name_part, max_name_width);
                let line_text = format!("{padded_name}  {}{}", command.description, hint_suffix);

                let max_width = ctx.size.width as usize;
                let truncated = truncate_text(&line_text, max_width);

                if is_selected {
                    let mut line = Line::with_style(truncated, ctx.theme.selected_row_style());
                    line.extend_bg_to_width(max_width);
                    line
                } else {
                    build_styled_command_line(&truncated, padded_name.len(), ctx.theme.muted())
                }
            });
        lines.extend(item_lines);

        lines
    }
}

fn build_styled_command_line(
    truncated: &str,
    name_byte_len: usize,
    muted: crate::tui::Color,
) -> Line {
    if truncated.len() <= name_byte_len {
        Line::new(truncated)
    } else {
        let mut line = Line::new(&truncated[..name_byte_len]);
        line.push_with_style(&truncated[name_byte_len..], Style::fg(muted));
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_picker::{
        rendered_lines_from, rendered_lines_with_context, rendered_raw_lines_with_context,
        type_query,
    };
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};
    use crate::tui::{Span, ViewContext, display_width_text};

    const DEFAULT_SIZE: (u16, u16) = (120, 40);

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

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

    fn selected_text(picker: &CommandPicker) -> Option<String> {
        let context = ViewContext::new(DEFAULT_SIZE);
        let lines = picker.render(&context);
        lines
            .iter()
            .find(|line| line.plain_text().starts_with("▶ "))
            .map(|line| line.plain_text())
    }

    #[test]
    fn init_shows_all_commands() {
        let picker = CommandPicker::new(sample_commands());
        let lines = rendered_lines_from(&picker.render(&ViewContext::new(DEFAULT_SIZE)));
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().any(|l| l.contains("/config")));
        assert!(lines.iter().any(|l| l.contains("/search")));
        assert!(lines.iter().any(|l| l.contains("/web")));
    }

    #[test]
    fn query_filters_by_name() {
        let mut picker = CommandPicker::new(sample_commands());
        type_query(&mut picker, "conf");
        let lines = rendered_lines_from(&picker.render(&ViewContext::new(DEFAULT_SIZE)));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("/config"));
    }

    #[test]
    fn query_filters_by_description() {
        let mut picker = CommandPicker::new(sample_commands());
        type_query(&mut picker, "browse");
        let lines = rendered_lines_from(&picker.render(&ViewContext::new(DEFAULT_SIZE)));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("/web"));
    }

    #[test]
    fn selection_wraps() {
        let mut picker = CommandPicker::new(sample_commands());
        let first = selected_text(&picker).unwrap();

        picker.on_event(&Event::Key(key(KeyCode::Up)));
        let last = selected_text(&picker).unwrap();
        assert_ne!(first, last);

        picker.on_event(&Event::Key(key(KeyCode::Down)));
        let back_to_first = selected_text(&picker).unwrap();
        assert_eq!(first, back_to_first);
    }

    #[test]
    fn selected_command_changes_on_move() {
        let mut picker = CommandPicker::new(sample_commands());
        let first = selected_text(&picker).unwrap();
        picker.on_event(&Event::Key(key(KeyCode::Down)));
        let second = selected_text(&picker).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn type_and_delete_updates_query() {
        let mut picker = CommandPicker::new(sample_commands());
        type_query(&mut picker, "co");
        assert_eq!(picker.query(), "co");

        picker.on_event(&Event::Key(key(KeyCode::Backspace)));
        assert_eq!(picker.query(), "c");

        picker.on_event(&Event::Key(key(KeyCode::Backspace)));
        assert_eq!(picker.query(), "");
    }

    #[test]
    fn render_includes_hint_for_commands_with_hint() {
        let picker = CommandPicker::new(sample_commands());
        let lines = rendered_lines_from(&picker.render(&ViewContext::new(DEFAULT_SIZE)));

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
        let picker = CommandPicker::new(sample_commands());
        let lines = rendered_lines_from(&picker.render(&ViewContext::new(DEFAULT_SIZE)));

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
    fn selected_entry_has_highlight_background() {
        let picker = CommandPicker::new(sample_commands());
        let context = ViewContext::new((80, 24));
        let lines = picker.render(&context);
        let selected_line = lines
            .iter()
            .find(|line| line.plain_text().starts_with("▶ "))
            .expect("should render a selected line");

        let has_bg = selected_line
            .spans()
            .iter()
            .any(|span| span.style().bg == Some(context.theme.highlight_bg()));
        assert!(has_bg, "selected entry should have highlight background");
    }

    #[test]
    fn selected_entry_has_text_primary_foreground() {
        let picker = CommandPicker::new(sample_commands());
        let context = ViewContext::new((80, 24));
        let lines = rendered_raw_lines_with_context(|ctx| picker.render(ctx), (80, 24));
        let selected_line = lines
            .iter()
            .find(|line| line.plain_text().starts_with("▶ "))
            .expect("should render a selected line");

        let has_fg = selected_line
            .spans()
            .iter()
            .any(|span| span.style().fg == Some(context.theme.text_primary()));
        assert!(has_fg, "selected entry should have text_primary foreground");
    }

    #[test]
    fn selected_entry_highlight_fills_full_line_width() {
        let picker = CommandPicker::new(sample_commands());
        let context = ViewContext::new((30, 24));
        let lines = rendered_raw_lines_with_context(|ctx| picker.render(ctx), (30, 24));
        let selected_line = lines
            .iter()
            .find(|line| line.plain_text().starts_with("▶ "))
            .expect("should render a selected line");

        assert_eq!(
            selected_line.display_width(),
            context.size.width as usize,
            "selected row should fill the full visible width",
        );
    }

    #[test]
    fn handle_key_enter_returns_selected_command() {
        let mut picker = CommandPicker::new(sample_commands());

        let outcome = picker.on_event(&Event::Key(key(KeyCode::Enter)));

        assert!(outcome.is_some());
        assert!(matches!(
            outcome.unwrap().as_slice(),
            [PickerMessage::Confirm(_)]
        ));
    }

    #[test]
    fn handle_key_backspace_on_empty_query_requests_close() {
        let mut picker = CommandPicker::new(sample_commands());

        let outcome = picker.on_event(&Event::Key(key(KeyCode::Backspace)));

        assert!(outcome.is_some());

        assert!(matches!(
            outcome.unwrap().as_slice(),
            [PickerMessage::CloseAndPopChar]
        ));
    }

    #[test]
    fn non_selected_items_have_multi_span_styling() {
        let picker = CommandPicker::new(sample_commands());
        let raw_lines = rendered_raw_lines_with_context(|ctx| picker.render(ctx), DEFAULT_SIZE);

        let non_selected = raw_lines
            .iter()
            .find(|l| l.plain_text().starts_with("  /"))
            .expect("should have a non-selected command line");

        // Non-selected items should have multiple spans with different styles:
        // the name part in default style and the description in muted style
        assert!(
            non_selected.spans().len() >= 2,
            "Non-selected item should have multiple spans for different styling, \
             got {} span(s): {:?}",
            non_selected.spans().len(),
            non_selected
                .spans()
                .iter()
                .map(Span::text)
                .collect::<Vec<_>>(),
        );

        let first_style = non_selected.spans()[0].style();
        let last_style = non_selected.spans().last().unwrap().style();
        assert_ne!(
            first_style, last_style,
            "Name and description spans should have different styles",
        );
    }

    #[test]
    fn descriptions_are_column_aligned() {
        let picker = CommandPicker::new(sample_commands());
        let lines = rendered_lines_from(&picker.render(&ViewContext::new(DEFAULT_SIZE)));

        let command_lines: Vec<&str> = lines.iter().map(std::string::String::as_str).collect();
        assert_eq!(command_lines.len(), 3);

        // All descriptions should start at the same display column.
        // Find the display column where the description text begins for each line.
        let desc_positions: Vec<usize> = sample_commands()
            .iter()
            .zip(command_lines.iter())
            .map(|(cmd, line)| {
                let byte_pos = line.find(&cmd.description).unwrap_or_else(|| {
                    panic!("description '{}' not found in '{}'", cmd.description, line)
                });
                display_width_text(&line[..byte_pos])
            })
            .collect();

        assert!(
            desc_positions.windows(2).all(|w| w[0] == w[1]),
            "Descriptions should start at the same column, but positions are: {desc_positions:?}\nLines: {command_lines:?}",
        );
    }

    #[test]
    fn long_commands_are_truncated_to_terminal_width() {
        let commands = vec![CommandEntry {
            name: "verylongcommandnamethatgoesonandon".into(),
            description: "This is a very long description that would normally wrap to multiple lines if we didn't truncate it".into(),
            has_input: false,
            hint: Some("some hint text".into()),
            builtin: false,
        }];

        let picker = CommandPicker::new(commands);
        let lines = rendered_lines_with_context(|ctx| picker.render(ctx), (30, 10));
        let command_line = &lines[0];

        assert_eq!(lines.len(), 1);
        assert!(
            command_line.ends_with("..."),
            "Expected truncation, got: {command_line}"
        );

        let width = display_width_text(command_line);
        assert!(
            width <= 30,
            "Line width {width} exceeds terminal width 30: {command_line}"
        );
    }

    #[test]
    fn handle_key_char_returns_char_typed() {
        let mut picker = CommandPicker::new(sample_commands());

        let outcome = picker.on_event(&Event::Key(key(KeyCode::Char('r'))));

        assert!(outcome.is_some());

        assert!(matches!(
            outcome.unwrap().as_slice(),
            [PickerMessage::CharTyped('r')]
        ));
        assert_eq!(picker.query(), "r");
    }

    #[test]
    fn handle_key_whitespace_closes_picker() {
        let mut picker = CommandPicker::new(sample_commands());

        let outcome = picker.on_event(&Event::Key(key(KeyCode::Char(' '))));

        assert!(outcome.is_some());

        assert!(matches!(
            outcome.unwrap().as_slice(),
            [PickerMessage::CloseWithChar(' ')]
        ));
    }

    #[test]
    fn handle_key_backspace_with_query_returns_pop_char() {
        let mut picker = CommandPicker::new(sample_commands());
        type_query(&mut picker, "co");

        let outcome = picker.on_event(&Event::Key(key(KeyCode::Backspace)));

        assert!(outcome.is_some());

        assert!(matches!(
            outcome.unwrap().as_slice(),
            [PickerMessage::PopChar]
        ));
        assert_eq!(picker.query(), "c");
    }
}
