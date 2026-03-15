use crate::components::git_diff_view::{GitDiffView, GitDiffViewMessage, build_patch_lines};
use crate::git_diff::{FileDiff, GitDiffDocument, PatchLineKind};
#[cfg(test)]
use crate::tui::MouseEvent;
use crate::tui::{Component, Event, Line, MouseEventKind, ViewContext};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenMode {
    Conversation,
    GitDiff,
}

pub enum GitDiffLoadState {
    Loading,
    Ready(GitDiffDocument),
    Empty,
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchFocus {
    FileList,
    Patch,
    CommentInput,
}

#[derive(Debug, Clone)]
pub struct PatchLineRef {
    pub hunk_index: usize,
    pub line_index: usize,
}

#[derive(Debug, Clone)]
pub struct QueuedComment {
    pub file_path: String,
    pub hunk_index: usize,
    pub hunk_text: String,
    pub line_text: String,
    pub line_number: Option<usize>,
    pub line_kind: PatchLineKind,
    pub comment: String,
}

pub struct GitDiffViewState {
    pub(crate) load_state: GitDiffLoadState,
    pub(crate) focus: PatchFocus,
    pub(crate) selected_file: usize,
    pub(crate) patch_scroll: usize,
    pub(crate) cached_patch_lines: Vec<Line>,
    pub(crate) cached_patch_line_refs: Vec<Option<PatchLineRef>>,
    pub(crate) cursor_line: usize,
    pub(crate) comment_buffer: String,
    pub(crate) comment_cursor: usize,
    pub(crate) queued_comments: Vec<QueuedComment>,
    cached_for_file: Option<usize>,
}

impl GitDiffViewState {
    pub fn new(load_state: GitDiffLoadState) -> Self {
        Self {
            load_state,
            focus: PatchFocus::FileList,
            selected_file: 0,
            patch_scroll: 0,
            cached_patch_lines: Vec::new(),
            cached_patch_line_refs: Vec::new(),
            cursor_line: 0,
            comment_buffer: String::new(),
            comment_cursor: 0,
            queued_comments: Vec::new(),
            cached_for_file: None,
        }
    }

    pub(crate) fn invalidate_patch_cache(&mut self) {
        self.cached_for_file = None;
        self.cached_patch_lines.clear();
        self.cached_patch_line_refs.clear();
    }

    pub(crate) fn selected_file(&self) -> Option<&FileDiff> {
        let GitDiffLoadState::Ready(doc) = &self.load_state else {
            return None;
        };
        doc.files
            .get(self.selected_file.min(doc.files.len().saturating_sub(1)))
    }

    pub(crate) fn selected_file_path(&self) -> Option<&str> {
        self.selected_file().map(|file| file.path.as_str())
    }

    pub(crate) fn file_count(&self) -> usize {
        match &self.load_state {
            GitDiffLoadState::Ready(doc) => doc.files.len(),
            _ => 0,
        }
    }

    pub(crate) fn max_patch_scroll(&self) -> usize {
        if let Some(file) = self.selected_file() {
            let total_lines = self.cached_patch_lines.len().max(
                file.hunks.iter().map(|h| h.lines.len()).sum::<usize>()
                    + file.hunks.len().saturating_sub(1),
            );
            return total_lines.saturating_sub(1);
        }
        0
    }

    pub(crate) fn selected_hunk_offsets(&self) -> Vec<usize> {
        let Some(file) = self.selected_file() else {
            return Vec::new();
        };
        let mut offsets = Vec::with_capacity(file.hunks.len());
        let mut offset = 0;
        for hunk in &file.hunks {
            offsets.push(offset);
            offset += hunk.lines.len() + 1;
        }
        offsets
    }

    pub(crate) fn set_focus(&mut self, focus: PatchFocus) -> bool {
        if self.focus == focus {
            return false;
        }
        self.focus = focus;
        if focus == PatchFocus::Patch {
            self.cursor_line = 0;
            self.patch_scroll = 0;
        }
        true
    }

    pub(crate) fn select_relative(&mut self, delta: isize) -> bool {
        if self.focus != PatchFocus::FileList {
            return false;
        }
        let file_count = self.file_count();
        if file_count == 0 {
            return false;
        }

        let previous = self.selected_file;
        crate::components::wrap_selection(&mut self.selected_file, file_count, delta);
        let changed = self.selected_file != previous;
        if changed {
            self.cursor_line = 0;
            self.patch_scroll = 0;
        }
        changed
    }

    pub(crate) fn move_cursor(&mut self, delta: isize) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }
        let max = self.max_patch_scroll();
        let next = if delta.is_negative() {
            self.cursor_line.saturating_sub(delta.unsigned_abs())
        } else {
            (self.cursor_line + delta.unsigned_abs()).min(max)
        };
        let changed = next != self.cursor_line;
        self.cursor_line = next;
        changed
    }

    #[allow(dead_code)]
    pub(crate) fn scroll_patch(&mut self, delta: isize) -> bool {
        let max = self.max_patch_scroll();
        let next = if delta.is_negative() {
            self.patch_scroll.saturating_sub(delta.unsigned_abs())
        } else {
            (self.patch_scroll + delta.unsigned_abs()).min(max)
        };
        let changed = next != self.patch_scroll;
        self.patch_scroll = next;
        changed
    }

    pub(crate) fn move_cursor_to_start(&mut self) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }
        let changed = self.cursor_line != 0;
        self.cursor_line = 0;
        changed
    }

    pub(crate) fn move_cursor_to_end(&mut self) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }
        let next = self.max_patch_scroll();
        let changed = next != self.cursor_line;
        self.cursor_line = next;
        changed
    }

    pub(crate) fn jump_next_hunk(&mut self) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }
        let current = self.cursor_line;
        if let Some(&next) = self.selected_hunk_offsets().iter().find(|&&o| o > current) {
            let next = next.min(self.max_patch_scroll());
            let changed = next != self.cursor_line;
            self.cursor_line = next;
            return changed;
        }
        false
    }

    pub(crate) fn jump_prev_hunk(&mut self) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }
        let current = self.cursor_line;
        if let Some(&prev) = self
            .selected_hunk_offsets()
            .iter()
            .rev()
            .find(|&&o| o < current)
        {
            let changed = prev != self.cursor_line;
            self.cursor_line = prev;
            return changed;
        }
        false
    }

    pub(crate) fn ensure_cursor_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        if self.cursor_line < self.patch_scroll {
            self.patch_scroll = self.cursor_line;
        } else if self.cursor_line >= self.patch_scroll + viewport_height {
            self.patch_scroll = self.cursor_line.saturating_sub(viewport_height - 1);
        }
    }

    pub(crate) fn ensure_patch_cache(&mut self, context: &ViewContext) {
        if self.cached_for_file == Some(self.selected_file) {
            return;
        }

        let Some(file) = self.selected_file() else {
            return;
        };

        if file.binary {
            self.cached_patch_lines = Vec::new();
            self.cached_patch_line_refs = Vec::new();
        } else {
            let (lines, refs) = build_patch_lines(file, context);
            self.cached_patch_lines = lines;
            self.cached_patch_line_refs = refs;
        }
        self.cached_for_file = Some(self.selected_file);
    }

    #[allow(dead_code)]
    fn apply_loaded_document(&mut self, doc: GitDiffDocument, restore: Option<RefreshState>) {
        if doc.files.is_empty() {
            self.load_state = GitDiffLoadState::Empty;
            self.invalidate_patch_cache();
            return;
        }

        let file_count = doc.files.len();
        self.load_state = GitDiffLoadState::Ready(doc);
        self.selected_file = self.selected_file.min(file_count.saturating_sub(1));
        self.invalidate_patch_cache();

        if let Some(restore) = restore {
            self.focus = restore.focus;
            self.patch_scroll = 0;
            if let (Some(path), GitDiffLoadState::Ready(doc)) =
                (&restore.selected_path, &self.load_state)
            {
                self.selected_file = doc
                    .files
                    .iter()
                    .position(|file| file.path == *path)
                    .unwrap_or(0);
            }
        }
    }
}

#[allow(dead_code)]
struct RefreshState {
    selected_path: Option<String>,
    focus: PatchFocus,
}

#[allow(dead_code)]
pub struct GitDiffMode {
    working_dir: PathBuf,
    cached_repo_root: Option<PathBuf>,
    state: GitDiffViewState,
    pending_restore: Option<RefreshState>,
}

impl GitDiffMode {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            cached_repo_root: None,
            state: GitDiffViewState::new(GitDiffLoadState::Empty),
            pending_restore: None,
        }
    }

    pub(crate) fn begin_open(&mut self) {
        self.pending_restore = None;
        self.state = GitDiffViewState::new(GitDiffLoadState::Loading);
    }

    pub(crate) fn begin_refresh(&mut self) {
        self.pending_restore = Some(RefreshState {
            selected_path: self.state.selected_file_path().map(ToOwned::to_owned),
            focus: self.state.focus,
        });
        self.state.load_state = GitDiffLoadState::Loading;
        self.state.invalidate_patch_cache();
    }

    #[allow(dead_code)]
    pub(crate) async fn complete_load(&mut self) {
        match crate::git_diff::load_git_diff(&self.working_dir, self.cached_repo_root.as_deref())
            .await
        {
            Ok(doc) => {
                if self.cached_repo_root.is_none() {
                    self.cached_repo_root = Some(doc.repo_root.clone());
                }
                self.state
                    .apply_loaded_document(doc, self.pending_restore.take());
            }
            Err(error) => {
                self.pending_restore = None;
                self.state.load_state = GitDiffLoadState::Error {
                    message: error.to_string(),
                };
                self.state.invalidate_patch_cache();
            }
        }
    }

    pub(crate) fn close(&mut self) {
        self.pending_restore = None;
        self.state = GitDiffViewState::new(GitDiffLoadState::Empty);
    }

    pub(crate) fn on_key_event(&mut self, event: &Event) -> Vec<GitDiffViewMessage> {
        let mut view = GitDiffView {
            state: &mut self.state,
        };
        let outcome = view.on_event(event);
        outcome.unwrap_or_default()
    }

    #[allow(dead_code)]
    pub(crate) fn on_mouse_event(&mut self, event: &Event) {
        if let Event::Mouse(mouse) = event {
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    self.state.scroll_patch(-3);
                }
                MouseEventKind::ScrollDown => {
                    self.state.scroll_patch(3);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn render_lines(&self, context: &ViewContext) -> Vec<Line> {
        GitDiffView::render_from_state(&self.state, context)
    }

    pub(crate) fn refresh_caches(&mut self, context: &ViewContext) {
        self.state.ensure_patch_cache(context);
        let viewport_height = (context.size.height as usize).saturating_sub(2);
        self.state.ensure_cursor_visible(viewport_height);
    }

    pub(crate) fn is_comment_input(&self) -> bool {
        self.state.focus == PatchFocus::CommentInput
    }

    pub(crate) fn comment_cursor_col(&self) -> usize {
        self.state.comment_cursor
    }
}

pub(crate) fn format_review_prompt(comments: &[QueuedComment]) -> String {
    use std::fmt::Write;

    let mut prompt = String::from("I'm reviewing the working tree diff. Here are my comments:\n");

    let mut file_groups: Vec<(&str, Vec<&QueuedComment>)> = Vec::new();
    for comment in comments {
        if let Some(group) = file_groups
            .iter_mut()
            .find(|(path, _)| *path == comment.file_path)
        {
            group.1.push(comment);
        } else {
            file_groups.push((&comment.file_path, vec![comment]));
        }
    }

    for (file_path, file_comments) in &file_groups {
        write!(prompt, "\n## `{file_path}`\n").unwrap();

        let mut hunk_groups: Vec<(usize, &str, Vec<&QueuedComment>)> = Vec::new();
        for comment in file_comments {
            if let Some(group) = hunk_groups
                .iter_mut()
                .find(|(idx, _, _)| *idx == comment.hunk_index)
            {
                group.2.push(comment);
            } else {
                hunk_groups.push((comment.hunk_index, &comment.hunk_text, vec![comment]));
            }
        }

        for (_, hunk_text, hunk_comments) in &hunk_groups {
            write!(prompt, "\n```diff\n{hunk_text}\n```\n").unwrap();

            for comment in hunk_comments {
                let kind_label = match comment.line_kind {
                    PatchLineKind::Added => "added",
                    PatchLineKind::Removed => "removed",
                    PatchLineKind::Context => "context",
                    PatchLineKind::HunkHeader => "header",
                    PatchLineKind::Meta => "meta",
                };
                let line_ref = match comment.line_number {
                    Some(n) => format!("Line {n} ({kind_label})"),
                    None => kind_label.to_string(),
                };
                write!(
                    prompt,
                    "\n**{line_ref}:** `{}`\n> {}\n",
                    comment.line_text, comment.comment
                )
                .unwrap();
            }
        }
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_diff::{FileStatus, Hunk, PatchLine, PatchLineKind};

    fn make_doc(paths: &[&str]) -> GitDiffDocument {
        GitDiffDocument {
            repo_root: PathBuf::from("/tmp/repo"),
            files: paths
                .iter()
                .map(|path| FileDiff {
                    old_path: None,
                    path: (*path).to_string(),
                    status: FileStatus::Modified,
                    hunks: vec![Hunk {
                        header: "@@ -1 +1 @@".to_string(),
                        old_start: 1,
                        old_count: 1,
                        new_start: 1,
                        new_count: 2,
                        lines: vec![
                            PatchLine {
                                kind: PatchLineKind::HunkHeader,
                                text: "@@ -1 +1 @@".to_string(),
                                old_line_no: None,
                                new_line_no: None,
                            },
                            PatchLine {
                                kind: PatchLineKind::Context,
                                text: "line one".to_string(),
                                old_line_no: Some(1),
                                new_line_no: Some(1),
                            },
                            PatchLine {
                                kind: PatchLineKind::Added,
                                text: "line two".to_string(),
                                old_line_no: None,
                                new_line_no: Some(2),
                            },
                        ],
                    }],
                    binary: false,
                })
                .collect(),
        }
    }

    #[test]
    fn begin_refresh_preserves_selected_path_and_focus_after_load() {
        let mut mode = GitDiffMode::new(PathBuf::from("."));
        mode.state.load_state = GitDiffLoadState::Ready(make_doc(&["a.rs", "b.rs"]));
        mode.state.selected_file = 1;
        mode.state.focus = PatchFocus::Patch;
        mode.begin_refresh();

        mode.state
            .apply_loaded_document(make_doc(&["c.rs", "b.rs"]), mode.pending_restore.take());

        assert_eq!(mode.state.selected_file_path(), Some("b.rs"));
        assert_eq!(mode.state.focus, PatchFocus::Patch);
        assert_eq!(mode.state.patch_scroll, 0);
    }

    #[test]
    fn mouse_scroll_moves_patch_scroll() {
        let mut mode = GitDiffMode::new(PathBuf::from("."));
        mode.state.load_state = GitDiffLoadState::Ready(make_doc(&["a.rs"]));
        mode.state.focus = PatchFocus::Patch;
        mode.state.cached_patch_lines = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        mode.on_mouse_event(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: crate::tui::KeyModifiers::NONE,
        }));
        assert_eq!(mode.state.patch_scroll, 2);
    }

    #[test]
    fn format_review_prompt_groups_by_file() {
        let comments = vec![
            QueuedComment {
                file_path: "src/foo.rs".to_string(),
                hunk_index: 0,
                hunk_text: "@@ -1,3 +1,3 @@\n fn main() {\n-    old();\n+    new();\n }"
                    .to_string(),
                line_text: "    new();".to_string(),
                line_number: Some(2),
                line_kind: PatchLineKind::Added,
                comment: "Looks risky".to_string(),
            },
            QueuedComment {
                file_path: "src/foo.rs".to_string(),
                hunk_index: 0,
                hunk_text: "@@ -1,3 +1,3 @@\n fn main() {\n-    old();\n+    new();\n }"
                    .to_string(),
                line_text: "    old();".to_string(),
                line_number: Some(2),
                line_kind: PatchLineKind::Removed,
                comment: "Why remove this?".to_string(),
            },
            QueuedComment {
                file_path: "src/bar.rs".to_string(),
                hunk_index: 0,
                hunk_text: "@@ -1 +1 @@\n+new_line".to_string(),
                line_text: "new_line".to_string(),
                line_number: Some(1),
                line_kind: PatchLineKind::Added,
                comment: "Needs a test".to_string(),
            },
        ];

        let prompt = format_review_prompt(&comments);
        assert!(
            prompt.contains("## `src/foo.rs`"),
            "should have foo.rs header"
        );
        assert!(
            prompt.contains("## `src/bar.rs`"),
            "should have bar.rs header"
        );
        assert_eq!(
            prompt.matches("```diff").count(),
            2,
            "one hunk per file group"
        );
        assert!(prompt.contains("Looks risky"));
        assert!(prompt.contains("Why remove this?"));
        assert!(prompt.contains("Needs a test"));
        assert!(prompt.contains("Line 2 (added)"));
        assert!(prompt.contains("Line 2 (removed)"));
        assert!(prompt.contains("Line 1 (added)"));
    }
}
