use crate::tui::{Combobox, Searchable};
use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;
use ignore::WalkBuilder;
use std::env::current_dir;
use std::path::{Path, PathBuf};

const MAX_INDEXED_FILES: usize = 50_000;

pub struct FilePicker {
    pub combobox: Combobox<FileMatch>,
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

fn should_exclude_path(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        value.starts_with('.') || matches!(value.as_ref(), "node_modules" | "target")
    })
}

impl Component for FilePicker {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();

        if self.combobox.matches.is_empty() {
            lines.push(Line::new("  (no matches found)".to_string()));
            return lines;
        }

        for (i, file) in self.combobox.matches.iter().enumerate() {
            let prefix = if i == self.combobox.selected_index {
                "▶ "
            } else {
                "  "
            };

            let line_text = format!("{}{}", prefix, file.display_name);
            let line = if i == self.combobox.selected_index {
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
}
