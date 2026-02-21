use crate::tui::{Combobox, Searchable};
use crate::tui::{Component, HandlesInput, InputOutcome, Line, RenderContext};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ignore::WalkBuilder;
use std::env::current_dir;
use std::path::{Path, PathBuf};

const MAX_INDEXED_FILES: usize = 50_000;

pub struct FilePicker {
    pub combobox: Combobox<FileMatch>,
}

pub enum FilePickerAction {
    Close,
    CloseAndPopChar,
    CloseWithChar(char),
    ConfirmSelection,
    CharTyped(char),
    PopChar,
}

#[derive(Debug, Clone)]
pub struct FileMatch {
    pub path: PathBuf,
    pub display_name: String,
}

impl Searchable for FileMatch {
    fn search_text(&self) -> String {
        self.display_name.clone()
    }
}

impl FilePicker {
    pub fn new() -> Self {
        let root = current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut entries = Vec::new();

        let walker = WalkBuilder::new(&root)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .hidden(false)
            .parents(true)
            .build();

        for entry in walker.flatten().take(MAX_INDEXED_FILES) {
            let path = entry.path();
            if !entry.file_type().is_some_and(|ft| ft.is_file()) || should_exclude_path(path) {
                continue;
            }

            let display_name = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");

            entries.push(FileMatch {
                path: path.to_path_buf(),
                display_name,
            });
        }

        entries.sort_by(|a, b| a.display_name.cmp(&b.display_name));

        Self {
            combobox: Combobox::new(entries),
        }
    }

    #[allow(dead_code)]
    pub fn from_matches(files: Vec<FileMatch>) -> Self {
        Self {
            combobox: Combobox::from_matches(files),
        }
    }

    #[cfg(test)]
    fn new_with_entries(entries: Vec<FileMatch>) -> Self {
        Self {
            combobox: Combobox::new(entries),
        }
    }

}

impl Default for FilePicker {
    fn default() -> Self {
        Self::new()
    }
}

fn should_exclude_path(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        value.starts_with('.') || matches!(value.as_ref(), "node_modules" | "target")
    })
}

impl Component for FilePicker {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();

        if self.combobox.matches.is_empty() {
            lines.push(Line::new("  (no matches found)".to_string()));
            return lines;
        }

        for (i, file) in self.combobox.visible_matches().iter().enumerate() {
            let prefix = if Some(i) == self.combobox.visible_selected_index() {
                "▶ "
            } else {
                "  "
            };

            let line_text = format!("{}{}", prefix, file.display_name);
            let line = if Some(i) == self.combobox.visible_selected_index() {
                Line::styled(line_text, context.theme.primary)
            } else {
                Line::new(line_text)
            };
            lines.push(line);
        }

        lines
    }
}

impl HandlesInput for FilePicker {
    type Action = FilePickerAction;

    fn handle_key(&mut self, key_event: KeyEvent) -> InputOutcome<Self::Action> {
        match key_event.code {
            KeyCode::Esc => InputOutcome::action_and_render(FilePickerAction::Close),
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
            KeyCode::Enter => InputOutcome::action_and_render(FilePickerAction::ConfirmSelection),
            KeyCode::Char(c) => {
                if c.is_whitespace() {
                    return InputOutcome::action_and_render(FilePickerAction::CloseWithChar(c));
                }
                self.combobox.push_query_char(c);
                InputOutcome::action_and_render(FilePickerAction::CharTyped(c))
            }
            KeyCode::Backspace => {
                if self.combobox.query.is_empty() {
                    InputOutcome::action_and_render(FilePickerAction::CloseAndPopChar)
                } else {
                    self.combobox.pop_query_char();
                    InputOutcome::action_and_render(FilePickerAction::PopChar)
                }
            }
            _ => InputOutcome::ignored(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn file_match(path: &str) -> FileMatch {
        FileMatch {
            path: PathBuf::from(path),
            display_name: path.to_string(),
        }
    }

    #[test]
    fn excludes_hidden_and_build_paths() {
        assert!(should_exclude_path(Path::new(".git/config")));
        assert!(should_exclude_path(Path::new(
            "node_modules/react/index.js"
        )));
        assert!(should_exclude_path(Path::new("target/debug/wisp")));
        assert!(should_exclude_path(Path::new("src/.cache/file.txt")));
        assert!(!should_exclude_path(Path::new("src/main.rs")));
    }

    #[test]
    fn query_filters_matches() {
        let mut picker = FilePicker::new_with_entries(vec![
            file_match("src/main.rs"),
            file_match("src/renderer.rs"),
            file_match("README.md"),
        ]);

        picker.combobox.update_query("rend".to_string());

        assert_eq!(picker.combobox.matches.len(), 1);
        assert_eq!(picker.combobox.matches[0].display_name, "src/renderer.rs");
    }

    #[test]
    fn selection_wraps() {
        let mut picker = FilePicker::new_with_entries(vec![
            file_match("a.rs"),
            file_match("b.rs"),
            file_match("c.rs"),
        ]);

        picker.combobox.move_selection_up();
        assert_eq!(picker.combobox.selected_index, 2);

        picker.combobox.move_selection_down();
        assert_eq!(picker.combobox.selected_index, 0);
    }

    #[test]
    fn handle_key_char_updates_query_and_returns_char_typed() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/renderer.rs")]);

        let outcome = picker.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(outcome.action, Some(FilePickerAction::CharTyped('r'))));
        assert_eq!(picker.combobox.query, "r");
    }

    #[test]
    fn handle_key_whitespace_closes_picker() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);

        let outcome = picker.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(
            outcome.action,
            Some(FilePickerAction::CloseWithChar(' '))
        ));
    }

    #[test]
    fn handle_key_enter_requests_confirmation() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);

        let outcome = picker.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(
            outcome.action,
            Some(FilePickerAction::ConfirmSelection)
        ));
    }

    #[test]
    fn backspace_with_empty_query_closes_and_pops() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);

        let outcome = picker.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(
            outcome.action,
            Some(FilePickerAction::CloseAndPopChar)
        ));
    }

    #[test]
    fn backspace_with_query_pops_char() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);
        picker.combobox.push_query_char('m');
        picker.combobox.push_query_char('a');

        let outcome = picker.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert!(outcome.consumed);
        assert!(outcome.needs_render);
        assert!(matches!(outcome.action, Some(FilePickerAction::PopChar)));
        assert_eq!(picker.combobox.query, "m");
    }
}
