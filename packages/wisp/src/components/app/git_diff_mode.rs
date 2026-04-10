use crate::components::file_list_panel::{FileListMessage, FileListPanel};
use crate::components::git_diff::git_diff_panel::{GitDiffPanel, GitDiffPanelMessage};
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
    document_revision: usize,
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
            document_revision: 0,
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
        self.split.right_mut().invalidate_diff_layer();
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
                self.split.right_mut().invalidate_diff_layer();
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

    pub async fn on_key_event(&mut self, event: &Event) -> Vec<GitDiffViewMessage> {
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
                    self.split.right_mut().invalidate_submitted_comments_layer();
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
                line.push_with_style(" ", Style::default().bg_color(theme.background()));
                if i == 0 {
                    line.push_with_style(&msg, Style::fg(theme.text_secondary()));
                }
                rows.push(line);
            }
            return Frame::new(rows);
        }

        self.prepare_right_panel_layers(context);
        self.split.set_separator_style(Style::default().bg_color(theme.background()));
        self.split.render(context)
    }

    fn prepare_right_panel_layers(&mut self, context: &ViewContext) {
        let GitDiffLoadState::Ready(doc) = &self.load_state else {
            return;
        };

        let selected = self.split.left().selected_file_index().unwrap_or(0).min(doc.files.len().saturating_sub(1));
        let file = &doc.files[selected];

        let file_comments =
            self.queued_comments.iter().filter(|comment| comment.file_path == file.path).collect::<Vec<_>>();

        let right_width = self.split.widths(context.size.width).right;
        self.split.right_mut().ensure_layers(file, &file_comments, right_width, self.document_revision);
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
        self.split.right_mut().invalidate_submitted_comments_layer();
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
        self.document_revision = self.document_revision.saturating_add(1);

        if doc.files.is_empty() {
            self.load_state = GitDiffLoadState::Empty;
            self.split.right_mut().invalidate_diff_layer();
            return;
        }

        self.split.left_mut().rebuild_from_files(&doc.files);
        self.split.right_mut().invalidate_diff_layer();

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
