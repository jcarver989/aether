use super::AppAction;
use crate::components::git_diff_view::{GitDiffView, GitDiffViewMessage, build_patch_lines};
use crate::git_diff::{FileDiff, GitDiffDocument};
use crate::tui::{
    Action, Component, InteractiveComponent, KeyEvent, Line, MessageResult, MouseEvent,
    MouseEventKind, RenderContext, UiEvent,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScreenMode {
    Conversation,
    GitDiff,
}

pub(crate) enum GitDiffLoadState {
    Loading,
    Ready(GitDiffDocument),
    Empty,
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatchFocus {
    FileList,
    Patch,
}

pub(crate) struct GitDiffViewState {
    pub(crate) load_state: GitDiffLoadState,
    pub(crate) focus: PatchFocus,
    pub(crate) selected_file: usize,
    pub(crate) patch_scroll: usize,
    pub(crate) cached_patch_lines: Vec<Line>,
    cached_for_file: Option<usize>,
}

impl GitDiffViewState {
    pub(crate) fn new(load_state: GitDiffLoadState) -> Self {
        Self {
            load_state,
            focus: PatchFocus::FileList,
            selected_file: 0,
            patch_scroll: 0,
            cached_patch_lines: Vec::new(),
            cached_for_file: None,
        }
    }

    pub(crate) fn invalidate_patch_cache(&mut self) {
        self.cached_for_file = None;
        self.cached_patch_lines.clear();
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
            self.patch_scroll = 0;
        }
        changed
    }

    pub(crate) fn scroll_patch(&mut self, delta: isize) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }

        let next = if delta.is_negative() {
            self.patch_scroll.saturating_sub(delta.unsigned_abs())
        } else {
            (self.patch_scroll + delta as usize).min(self.max_patch_scroll())
        };
        let changed = next != self.patch_scroll;
        self.patch_scroll = next;
        changed
    }

    pub(crate) fn scroll_patch_to_start(&mut self) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }
        let changed = self.patch_scroll != 0;
        self.patch_scroll = 0;
        changed
    }

    pub(crate) fn scroll_patch_to_end(&mut self) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }
        let next = self.max_patch_scroll();
        let changed = next != self.patch_scroll;
        self.patch_scroll = next;
        changed
    }

    pub(crate) fn jump_next_hunk(&mut self) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }
        let current = self.patch_scroll;
        if let Some(&next) = self.selected_hunk_offsets().iter().find(|&&o| o > current) {
            let next = next.min(self.max_patch_scroll());
            let changed = next != self.patch_scroll;
            self.patch_scroll = next;
            return changed;
        }
        false
    }

    pub(crate) fn jump_prev_hunk(&mut self) -> bool {
        if self.focus != PatchFocus::Patch {
            return false;
        }
        let current = self.patch_scroll;
        if let Some(&prev) = self
            .selected_hunk_offsets()
            .iter()
            .rev()
            .find(|&&o| o < current)
        {
            let changed = prev != self.patch_scroll;
            self.patch_scroll = prev;
            return changed;
        }
        false
    }

    pub(crate) fn ensure_patch_cache(&mut self, context: &RenderContext) {
        if self.cached_for_file == Some(self.selected_file) {
            return;
        }

        let Some(file) = self.selected_file() else {
            return;
        };

        self.cached_patch_lines = if file.binary {
            Vec::new()
        } else {
            build_patch_lines(file, context)
        };
        self.cached_for_file = Some(self.selected_file);
    }

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

struct RefreshState {
    selected_path: Option<String>,
    focus: PatchFocus,
}

pub(crate) struct GitDiffMode {
    working_dir: PathBuf,
    cached_repo_root: Option<PathBuf>,
    state: GitDiffViewState,
    pending_restore: Option<RefreshState>,
}

pub(crate) struct GitDiffModeInteraction {
    pub(crate) actions: Vec<Action<AppAction>>,
    pub(crate) changed: bool,
}

impl GitDiffModeInteraction {
    fn handled(actions: Vec<Action<AppAction>>, changed: bool) -> Self {
        Self { actions, changed }
    }
}

impl GitDiffMode {
    pub(crate) fn new(working_dir: PathBuf) -> Self {
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

    pub(crate) fn on_key_event(&mut self, key_event: KeyEvent) -> GitDiffModeInteraction {
        let mut view = GitDiffView {
            state: &mut self.state,
        };
        let outcome = view.on_event(UiEvent::Key(key_event));
        self.handle_messages(outcome)
    }

    pub(crate) fn on_mouse_event(&mut self, mouse: MouseEvent) -> bool {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.state.scroll_patch(-3),
            MouseEventKind::ScrollDown => self.state.scroll_patch(3),
            _ => false,
        }
    }

    pub(crate) fn prepare_render(&mut self, context: &RenderContext) {
        self.state.ensure_patch_cache(context);
    }

    pub(crate) fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        GitDiffView {
            state: &mut self.state,
        }
        .render(context)
    }

    fn handle_messages(
        &mut self,
        outcome: MessageResult<GitDiffViewMessage>,
    ) -> GitDiffModeInteraction {
        let changed = outcome.handled;
        let mut actions = Vec::new();

        for message in outcome.messages {
            match message {
                GitDiffViewMessage::Close => {
                    actions.push(Action::Custom(AppAction::CloseGitDiffViewer));
                }
                GitDiffViewMessage::Refresh => {
                    self.begin_refresh();
                    actions.push(Action::Custom(AppAction::RefreshGitDiffViewer));
                }
            }
        }

        GitDiffModeInteraction::handled(actions, changed)
    }
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
    fn mouse_scroll_only_moves_patch_focus() {
        let mut mode = GitDiffMode::new(PathBuf::from("."));
        mode.state.load_state = GitDiffLoadState::Ready(make_doc(&["a.rs"]));
        assert!(!mode.on_mouse_event(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: crate::tui::KeyModifiers::NONE,
        }));

        mode.state.focus = PatchFocus::Patch;
        mode.state.cached_patch_lines = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        assert!(mode.on_mouse_event(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: crate::tui::KeyModifiers::NONE,
        }));
        assert_eq!(mode.state.patch_scroll, 2);
    }
}
