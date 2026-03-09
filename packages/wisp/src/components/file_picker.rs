use crate::tui::{
    Combobox, Component, InteractiveComponent, Line, MessageResult, PickerKey, RenderContext,
    Searchable, UiEvent, classify_key,
};
use ignore::WalkBuilder;
use std::env::current_dir;
use std::path::{Path, PathBuf};

const MAX_INDEXED_FILES: usize = 50_000;

pub struct FilePicker {
    combobox: Combobox<FileMatch>,
}

pub enum FilePickerMessage {
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

    pub fn query(&self) -> &str {
        self.combobox.query()
    }

    pub fn selected(&self) -> Option<&FileMatch> {
        self.combobox.selected()
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
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();

        if self.combobox.is_empty() {
            lines.push(Line::new("  (no matches found)".to_string()));
            return lines;
        }

        let item_lines = self
            .combobox
            .render_items(context, |file, is_selected, ctx| {
                let prefix = if is_selected { "▶ " } else { "  " };
                let line_text = format!("{}{}", prefix, file.display_name);
                if is_selected {
                    let mut line = Line::with_style(line_text, ctx.theme.selected_row_style());
                    line.extend_bg_to_width(ctx.size.width as usize);
                    line
                } else {
                    Line::new(line_text)
                }
            });
        lines.extend(item_lines);

        lines
    }
}

impl InteractiveComponent for FilePicker {
    type Message = FilePickerMessage;

    fn on_event(&mut self, event: UiEvent) -> MessageResult<Self::Message> {
        match event {
            UiEvent::Key(key_event) => {
                match classify_key(key_event, self.combobox.query().is_empty()) {
                    PickerKey::Escape => MessageResult::message(FilePickerMessage::Close),
                    PickerKey::MoveUp => {
                        self.combobox.move_up();
                        MessageResult::consumed().with_render()
                    }
                    PickerKey::MoveDown => {
                        self.combobox.move_down();
                        MessageResult::consumed().with_render()
                    }
                    PickerKey::Confirm => {
                        MessageResult::message(FilePickerMessage::ConfirmSelection)
                    }
                    PickerKey::Char(c) => {
                        if c.is_whitespace() {
                            return MessageResult::message(FilePickerMessage::CloseWithChar(c));
                        }
                        self.combobox.push_query_char(c);
                        MessageResult::message(FilePickerMessage::CharTyped(c)).with_render()
                    }
                    PickerKey::Backspace => {
                        self.combobox.pop_query_char();
                        MessageResult::message(FilePickerMessage::PopChar).with_render()
                    }
                    PickerKey::BackspaceOnEmpty => {
                        MessageResult::message(FilePickerMessage::CloseAndPopChar)
                    }
                    PickerKey::MoveLeft
                    | PickerKey::MoveRight
                    | PickerKey::ControlChar
                    | PickerKey::Other => MessageResult::ignored(),
                }
            }
            UiEvent::Paste(_) | UiEvent::Tick(_) => MessageResult::ignored(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::RenderContext;
    use crate::tui::soft_wrap::display_width_line;
    use crate::tui::test_picker::{
        rendered_lines, rendered_raw_lines, rendered_raw_lines_with_size, selected_text, type_query,
    };
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};

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

        type_query(&mut picker, "rend");

        let lines = rendered_lines(&mut picker);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("src/renderer.rs"));
    }

    #[test]
    fn selection_wraps() {
        let mut picker = FilePicker::new_with_entries(vec![
            file_match("a.rs"),
            file_match("b.rs"),
            file_match("c.rs"),
        ]);

        let first = selected_text(&mut picker).unwrap();

        picker.on_event(UiEvent::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)));
        let last = selected_text(&mut picker).unwrap();
        assert_ne!(first, last);

        picker.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Down,
            KeyModifiers::NONE,
        )));
        let back_to_first = selected_text(&mut picker).unwrap();
        assert_eq!(first, back_to_first);
    }

    #[test]
    fn selected_entry_has_highlight_background() {
        let picker = FilePicker::new_with_entries(vec![
            file_match("a.rs"),
            file_match("b.rs"),
            file_match("c.rs"),
        ]);
        let context = RenderContext::new((80, 24));
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
        let mut picker = FilePicker::new_with_entries(vec![file_match("a.rs")]);
        let context = RenderContext::new((80, 24));
        let lines = rendered_raw_lines(&mut picker);
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
        let mut picker = FilePicker::new_with_entries(vec![file_match("a.rs")]);
        let context = RenderContext::new((20, 24));
        let lines = rendered_raw_lines_with_size(&mut picker, context.size);
        let selected_line = lines
            .iter()
            .find(|line| line.plain_text().starts_with("▶ "))
            .expect("should render a selected line");

        assert_eq!(
            display_width_line(selected_line),
            context.size.width as usize,
            "selected row should fill the full visible width",
        );
    }

    #[test]
    fn handle_key_char_updates_query_and_returns_char_typed() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/renderer.rs")]);

        let outcome = picker.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Char('r'),
            KeyModifiers::NONE,
        )));

        assert!(outcome.handled);

        assert!(matches!(
            outcome.messages.as_slice(),
            [FilePickerMessage::CharTyped('r')]
        ));
        assert_eq!(picker.query(), "r");
    }

    #[test]
    fn handle_key_whitespace_closes_picker() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);

        let outcome = picker.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Char(' '),
            KeyModifiers::NONE,
        )));

        assert!(outcome.handled);

        assert!(matches!(
            outcome.messages.as_slice(),
            [FilePickerMessage::CloseWithChar(' ')]
        ));
    }

    #[test]
    fn handle_key_enter_requests_confirmation() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);

        let outcome = picker.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));

        assert!(outcome.handled);

        assert!(matches!(
            outcome.messages.as_slice(),
            [FilePickerMessage::ConfirmSelection]
        ));
    }

    #[test]
    fn backspace_with_empty_query_closes_and_pops() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);

        let outcome = picker.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Backspace,
            KeyModifiers::NONE,
        )));

        assert!(outcome.handled);

        assert!(matches!(
            outcome.messages.as_slice(),
            [FilePickerMessage::CloseAndPopChar]
        ));
    }

    #[test]
    fn backspace_with_query_pops_char() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);
        type_query(&mut picker, "ma");

        let outcome = picker.on_event(UiEvent::Key(KeyEvent::new(
            KeyCode::Backspace,
            KeyModifiers::NONE,
        )));

        assert!(outcome.handled);

        assert!(matches!(
            outcome.messages.as_slice(),
            [FilePickerMessage::PopChar]
        ));
        assert_eq!(picker.query(), "m");
    }
}
