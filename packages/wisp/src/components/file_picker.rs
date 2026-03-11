use crate::tui::{
    Combobox, Line, PickerKey, PickerMessage, Response, Searchable, ViewContext, Widget,
    WidgetEvent, classify_key,
};
use ignore::WalkBuilder;
use std::env::current_dir;
use std::path::{Path, PathBuf};

const MAX_INDEXED_FILES: usize = 50_000;

pub struct FilePicker {
    combobox: Combobox<FileMatch>,
}

pub type FilePickerMessage = PickerMessage<FileMatch>;

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

impl Widget for FilePicker {
    type Message = FilePickerMessage;

    fn on_event(&mut self, event: &WidgetEvent) -> Response<Self::Message> {
        let WidgetEvent::Key(key_event) = event else {
            return Response::ignored();
        };
        match classify_key(*key_event, self.combobox.query().is_empty()) {
            PickerKey::Escape => Response::one(PickerMessage::Close),
            PickerKey::MoveUp => {
                self.combobox.move_up();
                Response::ok()
            }
            PickerKey::MoveDown => {
                self.combobox.move_down();
                Response::ok()
            }
            PickerKey::Confirm => {
                if let Some(selected) = self.combobox.selected().cloned() {
                    Response::one(PickerMessage::Confirm(selected))
                } else {
                    Response::one(PickerMessage::Close)
                }
            }
            PickerKey::Char(c) => {
                if c.is_whitespace() {
                    return Response::one(PickerMessage::CloseWithChar(c));
                }
                self.combobox.push_query_char(c);
                Response::one(PickerMessage::CharTyped(c))
            }
            PickerKey::Backspace => {
                self.combobox.pop_query_char();
                Response::one(PickerMessage::PopChar)
            }
            PickerKey::BackspaceOnEmpty => Response::one(PickerMessage::CloseAndPopChar),
            PickerKey::MoveLeft
            | PickerKey::MoveRight
            | PickerKey::ControlChar
            | PickerKey::Other => Response::ignored(),
        }
    }

    fn render(&self, context: &ViewContext) -> Vec<Line> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::ViewContext;
    use crate::tui::test_picker::{
        rendered_lines_from, rendered_raw_lines_with_context, type_query,
    };
    use crate::tui::{KeyCode, KeyEvent, KeyModifiers};

    const DEFAULT_SIZE: (u16, u16) = (120, 40);

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn file_match(path: &str) -> FileMatch {
        FileMatch {
            path: PathBuf::from(path),
            display_name: path.to_string(),
        }
    }

    fn selected_text(picker: &FilePicker) -> Option<String> {
        let context = ViewContext::new(DEFAULT_SIZE);
        let lines = picker.render(&context);
        lines
            .iter()
            .find(|line| line.plain_text().starts_with("▶ "))
            .map(|line| line.plain_text())
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

        let lines = rendered_lines_from(&picker.render(&ViewContext::new(DEFAULT_SIZE)));
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

        let first = selected_text(&picker).unwrap();

        picker.on_event(&WidgetEvent::Key(key(KeyCode::Up)));
        let last = selected_text(&picker).unwrap();
        assert_ne!(first, last);

        picker.on_event(&WidgetEvent::Key(key(KeyCode::Down)));
        let back_to_first = selected_text(&picker).unwrap();
        assert_eq!(first, back_to_first);
    }

    #[test]
    fn selected_entry_has_highlight_background() {
        let picker = FilePicker::new_with_entries(vec![
            file_match("a.rs"),
            file_match("b.rs"),
            file_match("c.rs"),
        ]);
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
        let picker = FilePicker::new_with_entries(vec![file_match("a.rs")]);
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
        let picker = FilePicker::new_with_entries(vec![file_match("a.rs")]);
        let context = ViewContext::new((20, 24));
        let lines = rendered_raw_lines_with_context(|ctx| picker.render(ctx), (20, 24));
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
    fn handle_key_char_updates_query_and_returns_char_typed() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/renderer.rs")]);

        let outcome = picker.on_event(&WidgetEvent::Key(key(KeyCode::Char('r'))));

        assert!(outcome.is_handled());

        assert!(matches!(
            outcome.into_messages().as_slice(),
            [PickerMessage::CharTyped('r')]
        ));
        assert_eq!(picker.query(), "r");
    }

    #[test]
    fn handle_key_whitespace_closes_picker() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);

        let outcome = picker.on_event(&WidgetEvent::Key(key(KeyCode::Char(' '))));

        assert!(outcome.is_handled());

        assert!(matches!(
            outcome.into_messages().as_slice(),
            [PickerMessage::CloseWithChar(' ')]
        ));
    }

    #[test]
    fn handle_key_enter_requests_confirmation() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);

        let outcome = picker.on_event(&WidgetEvent::Key(key(KeyCode::Enter)));

        assert!(outcome.is_handled());

        assert!(matches!(
            outcome.into_messages().as_slice(),
            [PickerMessage::Confirm(_)]
        ));
    }

    #[test]
    fn backspace_with_empty_query_closes_and_pops() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);

        let outcome = picker.on_event(&WidgetEvent::Key(key(KeyCode::Backspace)));

        assert!(outcome.is_handled());

        assert!(matches!(
            outcome.into_messages().as_slice(),
            [PickerMessage::CloseAndPopChar]
        ));
    }

    #[test]
    fn backspace_with_query_pops_char() {
        let mut picker = FilePicker::new_with_entries(vec![file_match("src/main.rs")]);
        type_query(&mut picker, "ma");

        let outcome = picker.on_event(&WidgetEvent::Key(key(KeyCode::Backspace)));

        assert!(outcome.is_handled());

        assert!(matches!(
            outcome.into_messages().as_slice(),
            [PickerMessage::PopChar]
        ));
        assert_eq!(picker.query(), "m");
    }
}
