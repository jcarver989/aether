use crate::components::file_list_panel::{FileListMessage, FileListPanel};
use crate::components::git_diff_panel::{GitDiffPanel, GitDiffPanelMessage};
use crate::git_diff::{GitDiffDocument, PatchLineKind, load_git_diff};
use std::path::PathBuf;
use tui::{Component, Either, Event, Frame, KeyCode, Line, SplitLayout, SplitPanel, Style, ViewContext};

pub enum GitDiffViewMessage {
    Close,
    Refresh,
    SubmitPrompt(String),
}

pub enum GitDiffLoadState {
    Loading,
    Ready(GitDiffDocument),
    Empty,
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatchLineRef {
    pub hunk_index: usize,
    pub line_index: usize,
}

#[derive(Debug, Clone)]
pub struct QueuedComment {
    pub file_path: String,
    pub patch_ref: PatchLineRef,
    pub line_text: String,
    pub line_number: Option<usize>,
    pub line_kind: PatchLineKind,
    pub comment: String,
}

pub struct GitDiffMode {
    working_dir: PathBuf,
    cached_repo_root: Option<PathBuf>,
    pub load_state: GitDiffLoadState,
    pub(crate) split: SplitPanel<FileListPanel, GitDiffPanel>,
    pub(crate) queued_comments: Vec<QueuedComment>,
    pending_restore: Option<RefreshState>,
}

impl GitDiffMode {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            cached_repo_root: None,
            load_state: GitDiffLoadState::Empty,
            split: SplitPanel::new(FileListPanel::new(), GitDiffPanel::new(), SplitLayout::fraction(1, 3, 20, 28))
                .with_separator(" ", Style::default())
                .with_resize_keys(),
            queued_comments: Vec::new(),
            pending_restore: None,
        }
    }

    pub(crate) fn begin_open(&mut self) {
        self.reset(GitDiffLoadState::Loading);
    }

    pub(crate) fn begin_refresh(&mut self) {
        self.pending_restore = Some(RefreshState {
            selected_path: self.selected_file_path().map(ToOwned::to_owned),
            was_right_focused: !self.split.is_left_focused(),
        });
        self.load_state = GitDiffLoadState::Loading;
        self.split.right_mut().invalidate_cache();
    }

    pub(crate) async fn complete_load(&mut self) {
        match load_git_diff(&self.working_dir, self.cached_repo_root.as_deref()).await {
            Ok(doc) => {
                if self.cached_repo_root.is_none() {
                    self.cached_repo_root = Some(doc.repo_root.clone());
                }
                let restore = self.pending_restore.take();
                self.apply_loaded_document(doc, restore);
            }
            Err(error) => {
                self.pending_restore = None;
                self.load_state = GitDiffLoadState::Error { message: error.to_string() };
                self.split.right_mut().invalidate_cache();
            }
        }
    }

    pub(crate) fn close(&mut self) {
        self.reset(GitDiffLoadState::Empty);
    }

    fn reset(&mut self, load_state: GitDiffLoadState) {
        self.pending_restore = None;
        self.load_state = load_state;
        *self.split.left_mut() = FileListPanel::new();
        *self.split.right_mut() = GitDiffPanel::new();
        self.queued_comments.clear();
        self.split.focus_left();
    }

    pub(crate) async fn on_key_event(&mut self, event: &Event) -> Vec<GitDiffViewMessage> {
        if self.split.right().is_in_comment_mode() {
            return self.on_comment_mode_event(event).await;
        }

        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Esc => return vec![GitDiffViewMessage::Close],
                KeyCode::Char('r') => return vec![GitDiffViewMessage::Refresh],
                KeyCode::Char('u') => {
                    self.queued_comments.pop();
                    self.split.left_mut().set_queued_comment_count(self.queued_comments.len());
                    self.split.right_mut().invalidate_cache();
                    return vec![];
                }
                KeyCode::Char('s') if !self.split.is_left_focused() => {
                    return self.submit_review();
                }
                KeyCode::Char('h') | KeyCode::Left if !self.split.is_left_focused() => {
                    self.split.focus_left();
                    return vec![];
                }
                _ => {}
            }
        }

        if let Some(msgs) = self.split.on_event(event).await {
            return self.handle_split_messages(msgs);
        }

        vec![]
    }

    pub fn render_frame(&mut self, context: &ViewContext) -> Frame {
        let theme = &context.theme;
        if context.size.width < 10 {
            return Frame::new(vec![Line::new("Too narrow")]);
        }

        let status_msg = match &self.load_state {
            GitDiffLoadState::Loading => Some("Loading...".to_string()),
            GitDiffLoadState::Empty => Some("No changes in working tree relative to HEAD".to_string()),
            GitDiffLoadState::Ready(doc) if doc.files.is_empty() => {
                Some("No changes in working tree relative to HEAD".to_string())
            }
            GitDiffLoadState::Error { message } => Some(format!("Git diff unavailable: {message}")),
            GitDiffLoadState::Ready(_) => None,
        };

        if let Some(msg) = status_msg {
            let height = context.size.height as usize;
            let widths = self.split.widths(context.size.width);
            let left_width = widths.left as usize;
            let mut rows = Vec::with_capacity(height);
            for i in 0..height {
                let mut line = Line::default();
                line.push_with_style(" ".repeat(left_width), Style::default().bg_color(theme.sidebar_bg()));
                line.push_with_style(" ", Style::default().bg_color(theme.code_bg()));
                if i == 0 {
                    line.push_with_style(&msg, Style::fg(theme.text_secondary()));
                }
                rows.push(line);
            }
            return Frame::new(rows);
        }

        self.prepare_right_panel_cache(context);
        self.split.set_separator_style(Style::default().bg_color(theme.code_bg()));
        self.split.render(context)
    }

    fn prepare_right_panel_cache(&mut self, context: &ViewContext) {
        let GitDiffLoadState::Ready(doc) = &self.load_state else {
            return;
        };

        let selected = self.split.left().selected_file_index().unwrap_or(0).min(doc.files.len().saturating_sub(1));
        let file = &doc.files[selected];

        let mut file_comments: Vec<QueuedComment> =
            self.queued_comments.iter().filter(|c| c.file_path == file.path).cloned().collect();
        if self.split.right().is_in_comment_mode()
            && let Some(draft) = self.split.right().build_draft_comment(file)
        {
            file_comments.push(draft);
        }

        let right_width = self.split.widths(context.size.width).right;
        self.split.right_mut().ensure_cache(file, &file_comments, right_width);
    }

    fn on_file_selected(&mut self, idx: usize) {
        self.split.left_mut().select_file_index(idx);
        self.split.right_mut().reset_for_new_file();
    }

    async fn on_comment_mode_event(&mut self, event: &Event) -> Vec<GitDiffViewMessage> {
        if let Some(msgs) = self.split.right_mut().on_event(event).await {
            return self.handle_right_panel_messages(msgs);
        }
        vec![]
    }

    fn handle_split_messages(
        &mut self,
        msgs: Vec<Either<FileListMessage, GitDiffPanelMessage>>,
    ) -> Vec<GitDiffViewMessage> {
        let mut right_msgs = Vec::new();
        for msg in msgs {
            match msg {
                Either::Left(FileListMessage::Selected(idx)) => {
                    self.on_file_selected(idx);
                }
                Either::Left(FileListMessage::FileOpened(idx)) => {
                    self.on_file_selected(idx);
                    self.split.focus_right();
                }
                Either::Right(panel_msg) => right_msgs.push(panel_msg),
            }
        }
        self.handle_right_panel_messages(right_msgs)
    }

    fn handle_right_panel_messages(&mut self, msgs: Vec<GitDiffPanelMessage>) -> Vec<GitDiffViewMessage> {
        for msg in msgs {
            let GitDiffPanelMessage::CommentSubmitted { anchor, text } = msg;
            self.queue_comment(anchor, &text);
        }
        vec![]
    }

    fn queue_comment(&mut self, anchor: PatchLineRef, text: &str) {
        let GitDiffLoadState::Ready(doc) = &self.load_state else {
            return;
        };
        let selected = self.split.left().selected_file_index().unwrap_or(0);
        let Some(file) = doc.files.get(selected) else {
            return;
        };
        let Some(hunk) = file.hunks.get(anchor.hunk_index) else {
            return;
        };
        let Some(patch_line) = hunk.lines.get(anchor.line_index) else {
            return;
        };

        self.queued_comments.push(QueuedComment {
            file_path: file.path.clone(),
            patch_ref: anchor,
            line_text: patch_line.text.clone(),
            line_number: patch_line.new_line_no.or(patch_line.old_line_no),
            line_kind: patch_line.kind,
            comment: text.to_string(),
        });
        self.split.left_mut().set_queued_comment_count(self.queued_comments.len());
        self.split.right_mut().invalidate_cache();
    }

    fn submit_review(&self) -> Vec<GitDiffViewMessage> {
        if self.queued_comments.is_empty() {
            return vec![];
        }
        let prompt = format_review_prompt(&self.queued_comments);
        vec![GitDiffViewMessage::SubmitPrompt(prompt)]
    }

    fn selected_file_path(&self) -> Option<&str> {
        let GitDiffLoadState::Ready(doc) = &self.load_state else {
            return None;
        };
        let idx = self.split.left().selected_file_index()?;
        doc.files.get(idx).map(|f| f.path.as_str())
    }

    pub fn load_document(&mut self, doc: GitDiffDocument) {
        self.apply_loaded_document(doc, None);
    }

    fn apply_loaded_document(&mut self, doc: GitDiffDocument, restore: Option<RefreshState>) {
        if doc.files.is_empty() {
            self.load_state = GitDiffLoadState::Empty;
            self.split.right_mut().invalidate_cache();
            return;
        }

        self.split.left_mut().rebuild_from_files(&doc.files);
        self.split.right_mut().invalidate_cache();

        if let Some(restore) = restore {
            if restore.was_right_focused {
                self.split.focus_right();
            } else {
                self.split.focus_left();
            }
            self.split.right_mut().reset_scroll();
            if let Some(path) = &restore.selected_path
                && let Some(idx) = doc.files.iter().position(|file| file.path == *path)
            {
                self.split.left_mut().select_file_index(idx);
            }
        }

        self.load_state = GitDiffLoadState::Ready(doc);
    }
}

struct RefreshState {
    selected_path: Option<String>,
    was_right_focused: bool,
}

pub(crate) fn format_review_prompt(comments: &[QueuedComment]) -> String {
    use std::fmt::Write;

    let mut prompt = String::from("I'm reviewing the working tree diff. Here are my comments:\n");

    let mut file_groups: Vec<(&str, Vec<&QueuedComment>)> = Vec::new();
    for comment in comments {
        if let Some(group) = file_groups.iter_mut().find(|(path, _)| *path == comment.file_path) {
            group.1.push(comment);
        } else {
            file_groups.push((&comment.file_path, vec![comment]));
        }
    }

    for (file_path, file_comments) in &file_groups {
        write!(prompt, "\n## `{file_path}`\n").unwrap();

        for comment in file_comments {
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
            write!(prompt, "\n**{line_ref}:** `{}`\n> {}\n", comment.line_text, comment.comment).unwrap();
        }
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_diff::{FileDiff, FileStatus, GitDiffDocument, Hunk, PatchLine, PatchLineKind};
    use tui::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, ViewContext};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn patch_line(kind: PatchLineKind, text: &str, old: Option<usize>, new: Option<usize>) -> PatchLine {
        PatchLine { kind, text: text.to_string(), old_line_no: old, new_line_no: new }
    }

    fn hunk(header: &str, old: (usize, usize), new: (usize, usize), lines: Vec<PatchLine>) -> Hunk {
        Hunk {
            header: header.to_string(),
            old_start: old.0,
            old_count: old.1,
            new_start: new.0,
            new_count: new.1,
            lines,
        }
    }

    fn file_diff(path: &str, old_path: Option<&str>, status: FileStatus, hunks: Vec<Hunk>) -> FileDiff {
        FileDiff { old_path: old_path.map(str::to_string), path: path.to_string(), status, hunks, binary: false }
    }

    fn make_test_doc() -> GitDiffDocument {
        use PatchLineKind::*;
        let h = "@@ -1,3 +1,3 @@";
        GitDiffDocument {
            repo_root: PathBuf::from("/tmp/test"),
            files: vec![
                file_diff(
                    "a.rs",
                    Some("a.rs"),
                    FileStatus::Modified,
                    vec![hunk(
                        h,
                        (1, 3),
                        (1, 3),
                        vec![
                            patch_line(HunkHeader, h, None, None),
                            patch_line(Context, "fn main() {", Some(1), Some(1)),
                            patch_line(Removed, "    old();", Some(2), None),
                            patch_line(Added, "    new();", None, Some(2)),
                            patch_line(Context, "}", Some(3), Some(3)),
                        ],
                    )],
                ),
                file_diff(
                    "b.rs",
                    None,
                    FileStatus::Added,
                    vec![hunk(
                        "@@ -0,0 +1,1 @@",
                        (0, 0),
                        (1, 1),
                        vec![
                            patch_line(HunkHeader, "@@ -0,0 +1,1 @@", None, None),
                            patch_line(Added, "new_content", None, Some(1)),
                        ],
                    )],
                ),
            ],
        }
    }

    fn make_mode(doc: GitDiffDocument) -> GitDiffMode {
        let mut mode = GitDiffMode::new(PathBuf::from("."));
        mode.apply_loaded_document(doc, None);
        mode
    }

    fn make_mode_with_cache() -> GitDiffMode {
        let mut mode = make_mode(make_test_doc());
        mode.render_frame(&ViewContext::new((100, 24)));
        mode
    }

    async fn send_key(mode: &mut GitDiffMode, code: KeyCode) -> Vec<GitDiffViewMessage> {
        mode.on_key_event(&Event::Key(key(code))).await
    }

    async fn send_mouse(mode: &mut GitDiffMode, kind: MouseEventKind) -> Vec<GitDiffViewMessage> {
        mode.on_key_event(&Event::Mouse(MouseEvent { kind, column: 0, row: 0, modifiers: KeyModifiers::NONE })).await
    }

    fn has_msg(msgs: &[GitDiffViewMessage], pred: fn(&GitDiffViewMessage) -> bool) -> bool {
        msgs.iter().any(pred)
    }

    fn queued_comment(line_text: &str, comment: &str, kind: PatchLineKind) -> QueuedComment {
        QueuedComment {
            file_path: "a.rs".to_string(),
            patch_ref: PatchLineRef { hunk_index: 0, line_index: 0 },
            line_text: line_text.to_string(),
            line_number: Some(1),
            line_kind: kind,
            comment: comment.to_string(),
        }
    }

    fn render_diff_text(mode: &mut GitDiffMode, width: u16) -> Vec<String> {
        let ctx = ViewContext::new((width, 24));
        mode.render_frame(&ctx).into_parts().0.iter().map(|l| l.plain_text()).collect()
    }

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

    fn mode_with(paths: &[&str]) -> GitDiffMode {
        let mut mode = GitDiffMode::new(PathBuf::from("."));
        let doc = make_doc(paths);
        mode.apply_loaded_document(doc, None);
        mode
    }

    fn comment(file: &str, line_text: &str, line_number: usize, kind: PatchLineKind, comment: &str) -> QueuedComment {
        QueuedComment {
            file_path: file.to_string(),
            patch_ref: PatchLineRef { hunk_index: 0, line_index: 0 },
            line_text: line_text.to_string(),
            line_number: Some(line_number),
            line_kind: kind,
            comment: comment.to_string(),
        }
    }

    #[test]
    fn begin_refresh_preserves_selected_path_and_focus_after_load() {
        let mut mode = mode_with(&["a.rs", "b.rs"]);
        mode.split.left_mut().select_file_index(1);
        mode.split.focus_right();
        mode.begin_refresh();

        let restore = mode.pending_restore.take();
        mode.apply_loaded_document(make_doc(&["c.rs", "b.rs"]), restore);

        assert_eq!(mode.selected_file_path(), Some("b.rs"));
        assert!(!mode.split.is_left_focused());
        assert_eq!(mode.split.right().scroll, 0);
    }

    #[test]
    fn format_review_prompt_groups_by_file() {
        let comments = vec![
            comment("src/foo.rs", "    new();", 2, PatchLineKind::Added, "Looks risky"),
            comment("src/foo.rs", "    old();", 2, PatchLineKind::Removed, "Why remove this?"),
            comment("src/bar.rs", "new_line", 1, PatchLineKind::Added, "Needs a test"),
        ];

        let prompt = format_review_prompt(&comments);
        assert!(prompt.contains("## `src/foo.rs`"), "should have foo.rs header");
        assert!(prompt.contains("## `src/bar.rs`"), "should have bar.rs header");
        assert!(!prompt.contains("```diff"), "should not include diff blocks");
        for expected in
            ["Looks risky", "Why remove this?", "Needs a test", "Line 2 (added)", "Line 2 (removed)", "Line 1 (added)"]
        {
            assert!(prompt.contains(expected), "missing: {expected}");
        }
    }

    #[test]
    fn narrow_terminal_renders_unified_diff() {
        let mut mode = make_mode(make_test_doc());
        let lines = render_diff_text(&mut mode, 108);
        assert!(lines.iter().any(|l| l.contains("old()")), "should contain removed line");
        assert!(lines.iter().any(|l| l.contains("new()")), "should contain added line");
        assert!(
            !lines.iter().any(|l| l.contains("old()") && l.contains("new()")),
            "unified mode: old and new should be on separate rows"
        );
    }

    #[test]
    fn wide_terminal_renders_split_diff() {
        let mut mode = make_mode(make_test_doc());
        let lines = render_diff_text(&mut mode, 109);
        assert!(
            lines.iter().any(|l| l.contains("old()") && l.contains("new()")),
            "split mode: old and new should appear on the same row"
        );
    }

    #[tokio::test]
    async fn resizing_split_panel_rebuilds_right_cache_for_new_width() {
        let mut mode = make_mode(make_test_doc());
        let ctx = ViewContext::new((130, 24));

        mode.render_frame(&ctx);
        send_key(&mut mode, KeyCode::Char('>')).await;

        let resized_right_width = usize::from(mode.split.widths(ctx.size.width).right);
        mode.render_frame(&ctx);

        assert!(
            mode.split.right().cached_lines.iter().all(|line| line.display_width() <= resized_right_width),
            "expected all cached lines to fit resized right width {resized_right_width}, got widths: {:?}",
            mode.split.right().cached_lines.iter().map(|line| line.display_width()).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn key_emits_expected_message() {
        let cases: Vec<(KeyCode, fn(&GitDiffViewMessage) -> bool)> = vec![
            (KeyCode::Esc, |m| matches!(m, GitDiffViewMessage::Close)),
            (KeyCode::Char('r'), |m| matches!(m, GitDiffViewMessage::Refresh)),
        ];
        for (code, pred) in cases {
            let mut mode = make_mode(make_test_doc());
            let msgs = send_key(&mut mode, code).await;
            assert!(has_msg(&msgs, pred), "failed for key: {code:?}");
        }
    }

    #[tokio::test]
    async fn j_and_k_move_file_selection() {
        let mut mode = make_mode(make_test_doc());
        assert_eq!(mode.split.left().selected_file_index(), Some(0));

        send_key(&mut mode, KeyCode::Char('j')).await;
        assert_eq!(mode.split.left().selected_file_index(), Some(1));

        let mut mode2 = make_mode(make_test_doc());
        send_key(&mut mode2, KeyCode::Char('k')).await;
        assert_eq!(mode2.split.left().selected_file_index(), Some(1));
    }

    #[tokio::test]
    async fn focus_switching() {
        let mut mode = make_mode(make_test_doc());
        assert!(mode.split.is_left_focused());
        send_key(&mut mode, KeyCode::Enter).await;
        assert!(!mode.split.is_left_focused());

        let mut mode2 = make_mode(make_test_doc());
        mode2.split.focus_right();
        send_key(&mut mode2, KeyCode::Char('h')).await;
        assert!(mode2.split.is_left_focused());
    }

    #[tokio::test]
    async fn file_selection_resets_patch_scroll() {
        let mut mode = make_mode(make_test_doc());
        mode.split.right_mut().scroll = 5;
        send_key(&mut mode, KeyCode::Char('j')).await;
        assert_eq!(mode.split.right().scroll, 0);
    }

    #[tokio::test]
    async fn c_enters_comment_mode() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.split.right_mut().cursor_line = 1;
        send_key(&mut mode, KeyCode::Char('c')).await;
        assert!(mode.split.right().is_in_comment_mode());
    }

    #[tokio::test]
    async fn c_on_spacer_is_noop() {
        use PatchLineKind::HunkHeader;
        let h1 = "@@ -1,1 +1,1 @@";
        let h2 = "@@ -5,1 +5,1 @@";
        let doc = GitDiffDocument {
            repo_root: PathBuf::from("/tmp/test"),
            files: vec![file_diff(
                "a.rs",
                None,
                FileStatus::Modified,
                vec![
                    hunk(h1, (1, 1), (1, 1), vec![patch_line(HunkHeader, h1, None, None)]),
                    hunk(h2, (5, 1), (5, 1), vec![patch_line(HunkHeader, h2, None, None)]),
                ],
            )],
        };
        let mut mode = make_mode(doc);
        mode.render_frame(&ViewContext::new((100, 24)));
        mode.split.focus_right();
        mode.split.right_mut().cursor_line = 1;

        send_key(&mut mode, KeyCode::Char('c')).await;
        assert!(!mode.split.right().is_in_comment_mode());
    }

    #[tokio::test]
    async fn esc_exits_comment_mode() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.split.right_mut().in_comment_mode = true;
        mode.split.right_mut().comment_buffer = "partial".to_string();
        send_key(&mut mode, KeyCode::Esc).await;
        assert!(!mode.split.right().is_in_comment_mode());
        assert!(mode.split.right().comment_buffer.is_empty());
    }

    #[tokio::test]
    async fn enter_queues_comment() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.split.right_mut().cursor_line = 1;
        send_key(&mut mode, KeyCode::Char('c')).await;
        assert!(mode.split.right().is_in_comment_mode());

        for ch in "test comment".chars() {
            send_key(&mut mode, KeyCode::Char(ch)).await;
        }
        assert_eq!(mode.split.right().comment_buffer, "test comment");

        send_key(&mut mode, KeyCode::Enter).await;
        assert!(!mode.split.right().is_in_comment_mode());
        assert_eq!(mode.queued_comments.len(), 1);
        assert_eq!(mode.queued_comments[0].comment, "test comment");
        assert!(mode.split.right().comment_buffer.is_empty());
    }

    #[tokio::test]
    async fn s_submits_review() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.queued_comments.push(queued_comment("line", "looks good", PatchLineKind::Context));
        let msgs = send_key(&mut mode, KeyCode::Char('s')).await;
        assert!(has_msg(&msgs, |m| matches!(m, GitDiffViewMessage::SubmitPrompt(_))));
        assert_eq!(mode.queued_comments.len(), 1, "submit should not clear queued comments before send is accepted");
    }

    #[tokio::test]
    async fn s_without_comments_is_noop() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        let msgs = send_key(&mut mode, KeyCode::Char('s')).await;
        assert!(msgs.is_empty());
    }

    #[tokio::test]
    async fn u_removes_last_comment() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.queued_comments.push(queued_comment("line1", "first", PatchLineKind::Context));
        mode.queued_comments.push(queued_comment("line2", "second", PatchLineKind::Added));
        send_key(&mut mode, KeyCode::Char('u')).await;
        assert_eq!(mode.queued_comments.len(), 1);
        assert_eq!(mode.queued_comments[0].comment, "first");
    }

    #[test]
    fn cursor_navigation_clamps() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.split.right_mut().cursor_line = 0;

        mode.split.right_mut().move_cursor(-1);
        assert_eq!(mode.split.right().cursor_line, 0);

        let max = mode.split.right().max_scroll();
        mode.split.right_mut().cursor_line = max;
        mode.split.right_mut().move_cursor(1);
        assert_eq!(mode.split.right().cursor_line, max);
    }

    #[tokio::test]
    async fn cursor_replaces_scroll() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.split.right_mut().cursor_line = 0;
        send_key(&mut mode, KeyCode::Char('j')).await;
        assert_eq!(mode.split.right().cursor_line, 1);
        send_key(&mut mode, KeyCode::Char('k')).await;
        assert_eq!(mode.split.right().cursor_line, 0);
    }

    #[tokio::test]
    async fn mouse_scroll_down_in_file_list_selects_next() {
        let mut mode = make_mode(make_test_doc());
        assert_eq!(mode.split.left().selected_file_index(), Some(0));
        send_mouse(&mut mode, MouseEventKind::ScrollDown).await;
        assert_eq!(mode.split.left().selected_file_index(), Some(1));
    }

    #[tokio::test]
    async fn mouse_scroll_up_in_patch_moves_cursor() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.split.right_mut().cursor_line = 4;
        send_mouse(&mut mode, MouseEventKind::ScrollUp).await;
        assert_eq!(mode.split.right().cursor_line, 1);
    }

    #[tokio::test]
    async fn mouse_scroll_during_comment_input_is_noop() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.split.right_mut().in_comment_mode = true;
        mode.split.right_mut().cursor_line = 2;
        let original_cursor = mode.split.right().cursor_line;
        let original_file = mode.split.left().selected_file_index();
        send_mouse(&mut mode, MouseEventKind::ScrollDown).await;
        assert_eq!(mode.split.right().cursor_line, original_cursor);
        assert_eq!(mode.split.left().selected_file_index(), original_file);
        assert!(mode.split.right().is_in_comment_mode());
    }

    #[test]
    fn hunk_offsets_account_for_soft_wrapped_lines() {
        use PatchLineKind::*;

        let long_line = "x".repeat(200);
        let h1 = "@@ -1,2 +1,2 @@";
        let h2 = "@@ -10,1 +10,1 @@";
        let doc = GitDiffDocument {
            repo_root: PathBuf::from("/tmp/test"),
            files: vec![file_diff(
                "a.rs",
                None,
                FileStatus::Modified,
                vec![
                    hunk(
                        h1,
                        (1, 2),
                        (1, 2),
                        vec![
                            patch_line(HunkHeader, h1, None, None),
                            patch_line(Added, &long_line, None, Some(1)),
                            patch_line(Context, "short", Some(2), Some(2)),
                        ],
                    ),
                    hunk(
                        h2,
                        (10, 1),
                        (10, 1),
                        vec![patch_line(HunkHeader, h2, None, None), patch_line(Context, "end", Some(10), Some(10))],
                    ),
                ],
            )],
        };

        let mut mode = make_mode(doc);
        mode.render_frame(&ViewContext::new((60, 24)));

        let offsets = mode.split.right().hunk_offsets();
        assert_eq!(offsets.len(), 2, "should find two hunks");

        let second_hunk_ref_pos = mode
            .split
            .right()
            .line_refs
            .iter()
            .position(|r| matches!(r, Some(r) if r.hunk_index == 1))
            .expect("hunk 1 should exist in refs");
        assert_eq!(offsets[1], second_hunk_ref_pos);

        assert!(
            offsets[1] > 4,
            "second hunk offset {} should exceed unwrapped count (4) due to soft-wrapping",
            offsets[1]
        );

        mode.split.focus_right();
        mode.split.right_mut().cursor_line = 0;
        assert!(mode.split.right_mut().jump_next_hunk());
        assert_eq!(mode.split.right().cursor_line, second_hunk_ref_pos);

        assert!(mode.split.right_mut().jump_prev_hunk());
        assert_eq!(mode.split.right().cursor_line, 0);
    }

    fn simple_hunks() -> Vec<Hunk> {
        use PatchLineKind::*;
        let h = "@@ -1,1 +1,1 @@";
        vec![hunk(
            h,
            (1, 1),
            (1, 1),
            vec![patch_line(HunkHeader, h, None, None), patch_line(Context, "line", Some(1), Some(1))],
        )]
    }

    fn make_tree_doc() -> GitDiffDocument {
        GitDiffDocument {
            repo_root: PathBuf::from("/tmp/test"),
            files: vec![
                file_diff("src/a.rs", None, FileStatus::Modified, simple_hunks()),
                file_diff("src/b.rs", None, FileStatus::Added, simple_hunks()),
                file_diff("lib/c.rs", None, FileStatus::Modified, simple_hunks()),
            ],
        }
    }

    fn make_tree_mode() -> GitDiffMode {
        make_mode(make_tree_doc())
    }

    #[tokio::test]
    async fn h_in_file_list_collapses_directory() {
        let mut mode = make_tree_mode();
        mode.split.left_mut().tree_mut().navigate(2);
        let entries_before = mode.split.left_mut().tree_mut().visible_entries().len();
        assert_eq!(entries_before, 5);

        send_key(&mut mode, KeyCode::Char('h')).await;

        let entries_after = mode.split.left_mut().tree_mut().visible_entries().len();
        assert_eq!(entries_after, 3);
    }

    #[tokio::test]
    async fn enter_on_directory_expands_it() {
        let mut mode = make_tree_mode();
        mode.split.left_mut().tree_mut().navigate(2);
        mode.split.left_mut().tree_collapse_or_parent();
        assert_eq!(mode.split.left_mut().tree_mut().visible_entries().len(), 3);

        send_key(&mut mode, KeyCode::Enter).await;

        assert!(mode.split.is_left_focused());
        assert_eq!(mode.split.left_mut().tree_mut().visible_entries().len(), 5);
    }

    #[tokio::test]
    async fn enter_on_file_switches_to_patch() {
        let mut mode = make_tree_mode();
        mode.split.left_mut().tree_mut().navigate(1);

        send_key(&mut mode, KeyCode::Enter).await;

        assert!(!mode.split.is_left_focused());
    }

    #[tokio::test]
    async fn enter_queues_comment_and_invalidates_cache() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.split.right_mut().cursor_line = 3;

        send_key(&mut mode, KeyCode::Char('c')).await;
        for ch in "test comment".chars() {
            send_key(&mut mode, KeyCode::Char(ch)).await;
        }
        send_key(&mut mode, KeyCode::Enter).await;

        assert_eq!(mode.queued_comments.len(), 1);
        assert_eq!(mode.queued_comments[0].comment, "test comment");

        assert!(mode.split.right().cached_lines.is_empty());
    }

    #[tokio::test]
    async fn undo_removes_last_comment_and_invalidates_cache() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.queued_comments.push(queued_comment("line1", "first", PatchLineKind::Context));
        mode.queued_comments.push(queued_comment("line2", "second", PatchLineKind::Added));

        send_key(&mut mode, KeyCode::Char('u')).await;

        assert_eq!(mode.queued_comments.len(), 1);
        assert_eq!(mode.queued_comments[0].comment, "first");

        assert!(mode.split.right().cached_lines.is_empty());
    }

    #[test]
    fn cursor_stays_on_logical_line_after_comment_insert() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        mode.split.right_mut().cursor_line = 3;
        let original_ref = mode.split.right().line_refs[3];

        mode.queued_comments.push(QueuedComment {
            file_path: "a.rs".to_string(),
            patch_ref: original_ref.unwrap(),
            line_text: "    new();".to_string(),
            line_number: Some(2),
            line_kind: PatchLineKind::Added,
            comment: "review".to_string(),
        });
        mode.split.right_mut().invalidate_cache();
        mode.render_frame(&ViewContext::new((100, 24)));

        let cursor = mode.split.right().cursor_line;
        let new_ref = mode.split.right().line_refs[cursor];
        assert_eq!(new_ref, original_ref, "cursor should stay on the same logical line");
        let line_text = mode.split.right().cached_lines[cursor].plain_text();
        assert!(line_text.contains("new();"), "cursor should be on the added line, got: {line_text}");
    }

    #[test]
    fn cursor_on_comment_row_restores_to_anchored_line() {
        let mut mode = make_mode_with_cache();
        mode.split.focus_right();
        let anchor = PatchLineRef { hunk_index: 0, line_index: 3 };
        mode.queued_comments.push(QueuedComment {
            file_path: "a.rs".to_string(),
            patch_ref: anchor,
            line_text: "    new();".to_string(),
            line_number: Some(2),
            line_kind: PatchLineKind::Added,
            comment: "review".to_string(),
        });
        mode.split.right_mut().invalidate_cache();
        mode.render_frame(&ViewContext::new((100, 24)));

        let comment_row = mode
            .split
            .right()
            .line_refs
            .iter()
            .position(|r: &Option<PatchLineRef>| r.is_none())
            .expect("should have a comment row");
        mode.split.right_mut().cursor_line = comment_row;

        mode.split.right_mut().invalidate_cache();
        mode.render_frame(&ViewContext::new((100, 24)));

        let cursor = mode.split.right().cursor_line;
        assert_eq!(
            mode.split.right().line_refs[cursor],
            Some(anchor),
            "cursor should be restored to the anchored diff line"
        );
    }
}
