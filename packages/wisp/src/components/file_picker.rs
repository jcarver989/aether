use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;
use ignore::WalkBuilder;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_VISIBLE_MATCHES: u32 = 10;
const MAX_INDEXED_FILES: usize = 50_000;
const MATCH_TIMEOUT_MS: u64 = 10;
const MAX_TICKS_PER_QUERY: usize = 4;

pub struct FilePicker {
    pub query: String,
    pub files: Vec<FileMatch>,
    pub selected_index: usize,
    search_engine: FileSearchEngine,
}

#[derive(Debug, Clone)]
pub struct FileMatch {
    pub path: PathBuf,
    pub display_name: String,
}

#[derive(Debug, Clone)]
struct IndexedFile {
    path: PathBuf,
    display_name: String,
}

struct FileSearchEngine {
    matcher: Nucleo<IndexedFile>,
}

impl FilePicker {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut picker = Self {
            query: String::new(),
            files: Vec::new(),
            selected_index: 0,
            search_engine: FileSearchEngine::from_root(&root),
        };
        picker.files = picker.search_engine.search("", false);
        picker
    }

    pub fn update_query(&mut self, query: String) {
        let append = query.starts_with(&self.query);
        self.query = query;
        self.files = self.search_engine.search(&self.query, append);
        if self.selected_index >= self.files.len() {
            self.selected_index = 0;
        }
    }

    pub fn move_selection_up(&mut self) {
        if !self.files.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.files.len() - 1;
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        if !self.files.is_empty() {
            if self.selected_index < self.files.len() - 1 {
                self.selected_index += 1;
            } else {
                self.selected_index = 0;
            }
        }
    }

    #[cfg(test)]
    fn new_with_entries(entries: Vec<IndexedFile>) -> Self {
        let mut picker = Self {
            query: String::new(),
            files: Vec::new(),
            selected_index: 0,
            search_engine: FileSearchEngine::from_entries(entries),
        };
        picker.files = picker.search_engine.search("", false);
        picker
    }
}

impl FileSearchEngine {
    fn from_root(root: &Path) -> Self {
        let mut entries = Vec::new();

        let walker = WalkBuilder::new(root)
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
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");

            entries.push(IndexedFile {
                path: path.to_path_buf(),
                display_name,
            });
        }

        Self::from_entries(entries)
    }

    fn from_entries(mut entries: Vec<IndexedFile>) -> Self {
        entries.sort_by(|a, b| a.display_name.cmp(&b.display_name));

        let mut matcher = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), Some(1), 1);
        let injector = matcher.injector();
        for entry in entries {
            injector.push(entry, |item, columns| {
                columns[0] = item.display_name.as_str().into();
            });
        }
        let _ = matcher.tick(0);
        Self { matcher }
    }

    fn search(&mut self, query: &str, append: bool) -> Vec<FileMatch> {
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
            .map(|item| FileMatch {
                path: item.data.path.clone(),
                display_name: item.data.display_name.clone(),
            })
            .collect()
    }
}

fn should_exclude_path(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        value.starts_with('.') || matches!(value.as_ref(), "node_modules" | "target")
    })
}

pub struct FilePickerComponent<'a> {
    pub picker: &'a FilePicker,
}

impl Component for FilePickerComponent<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let mut lines = Vec::new();

        if self.picker.files.is_empty() {
            lines.push(Line::new("  (no matches found)".to_string()));
            return lines;
        }

        for (i, file) in self.picker.files.iter().enumerate() {
            let prefix = if i == self.picker.selected_index {
                "▶ "
            } else {
                "  "
            };

            let line_text = format!("{}{}", prefix, file.display_name);
            let line = if i == self.picker.selected_index {
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

    fn indexed(path: &str) -> IndexedFile {
        IndexedFile {
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
            indexed("src/main.rs"),
            indexed("src/renderer.rs"),
            indexed("README.md"),
        ]);

        picker.update_query("rend".to_string());

        assert_eq!(picker.files.len(), 1);
        assert_eq!(picker.files[0].display_name, "src/renderer.rs");
    }

    #[test]
    fn selection_wraps() {
        let mut picker =
            FilePicker::new_with_entries(vec![indexed("a.rs"), indexed("b.rs"), indexed("c.rs")]);

        picker.move_selection_up();
        assert_eq!(picker.selected_index, 2);

        picker.move_selection_down();
        assert_eq!(picker.selected_index, 0);
    }
}
