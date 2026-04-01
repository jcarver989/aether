use crate::components::app::git_diff_mode::{QueuedComment, format_review_prompt};
use crate::components::app::{GitDiffLoadState, GitDiffViewState, PatchFocus};
use crate::components::file_list_renderer::{render_file_list_cell, render_file_tree_cell};
use crate::components::file_tree::FileTree;
pub use crate::components::patch_renderer::build_patch_lines;
use crate::git_diff::{FileDiff, FileStatus, PatchLineKind};
use tui::{Component, Event, Frame, KeyCode, Line, MouseEvent, MouseEventKind, Style, ViewContext, truncate_text};

pub enum GitDiffViewMessage {
    Close,
    Refresh,
    SubmitPrompt(String),
}

pub struct GitDiffView<'a> {
    pub state: &'a mut GitDiffViewState,
}

impl GitDiffView<'_> {
    pub fn render_from_state(state: &GitDiffViewState, context: &ViewContext) -> Vec<Line> {
        render_git_diff_state(state, context)
    }
}

pub(crate) fn diff_layout(total_width: usize, delta: i16) -> (usize, usize) {
    let base = (total_width / 3).clamp(20, 28).min(total_width.saturating_sub(4));
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let left = (base as i16 + delta).clamp(12, (total_width / 2) as i16) as usize;
    let right = total_width.saturating_sub(left + 1);
    (left, right)
}

pub(crate) fn should_use_split_patch(total_width: usize, delta: i16, file: &FileDiff) -> bool {
    let (_left_width, right_width) = diff_layout(total_width, delta);
    let has_removals = file.hunks.iter().flat_map(|h| &h.lines).any(|line| line.kind == PatchLineKind::Removed);

    right_width >= 80 && has_removals
}

fn render_git_diff_state(state: &GitDiffViewState, context: &ViewContext) -> Vec<Line> {
    let theme = &context.theme;
    let total_width = context.size.width as usize;
    if total_width < 10 {
        return vec![Line::new("Too narrow")];
    }

    let (left_width, right_width) = diff_layout(total_width, state.sidebar_width_delta);
    let available_height = context.size.height as usize;

    match &state.load_state {
        GitDiffLoadState::Loading => render_message_layout("Loading...", left_width, available_height, theme),
        GitDiffLoadState::Empty => {
            render_message_layout("No changes in working tree relative to HEAD", left_width, available_height, theme)
        }
        GitDiffLoadState::Error { message } => {
            let msg = format!("Git diff unavailable: {message}");
            render_message_layout(&msg, left_width, available_height, theme)
        }
        GitDiffLoadState::Ready(doc) if doc.files.is_empty() => {
            render_message_layout("No changes in working tree relative to HEAD", left_width, available_height, theme)
        }
        GitDiffLoadState::Ready(doc) => {
            render_ready(&doc.files, state, left_width, right_width, available_height, context)
        }
    }
}

impl Component for GitDiffView<'_> {
    type Message = GitDiffViewMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        match event {
            Event::Mouse(mouse) => Some(self.on_mouse_event(*mouse)),
            Event::Key(key) => Some(self.on_key_event(key.code)),
            _ => None,
        }
    }

    fn render(&mut self, context: &ViewContext) -> Frame {
        Frame::new(render_git_diff_state(self.state, context))
    }
}

impl GitDiffView<'_> {
    fn on_key_event(&mut self, code: KeyCode) -> Vec<GitDiffViewMessage> {
        if self.state.focus == PatchFocus::CommentInput {
            return self.on_comment_input(code);
        }

        match code {
            KeyCode::Esc => vec![GitDiffViewMessage::Close],
            KeyCode::Char('r') => vec![GitDiffViewMessage::Refresh],
            KeyCode::Char('h') | KeyCode::Left => {
                if self.state.focus == PatchFocus::FileList {
                    self.state.tree_collapse_or_parent();
                } else {
                    self.state.set_focus(PatchFocus::FileList);
                }
                vec![]
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                if self.state.focus == PatchFocus::FileList {
                    if self.state.tree_expand_or_enter() {
                        self.state.set_focus(PatchFocus::Patch);
                    }
                } else {
                    self.state.set_focus(PatchFocus::Patch);
                }
                vec![]
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.navigate_down();
                vec![]
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.navigate_up();
                vec![]
            }
            KeyCode::Char('g') => {
                self.state.move_cursor_to_start();
                vec![]
            }
            KeyCode::Char('G') => {
                self.state.move_cursor_to_end();
                vec![]
            }
            KeyCode::PageDown => {
                self.state.move_cursor(20);
                vec![]
            }
            KeyCode::PageUp => {
                self.state.move_cursor(-20);
                vec![]
            }
            KeyCode::Char('n') => {
                self.state.jump_next_hunk();
                vec![]
            }
            KeyCode::Char('p') => {
                self.state.jump_prev_hunk();
                vec![]
            }
            KeyCode::Char('c') => {
                self.enter_comment_mode();
                vec![]
            }
            KeyCode::Char('s') => self.submit_review(),
            KeyCode::Char('u') => {
                self.state.queued_comments.pop();
                self.state.invalidate_patch_cache();
                vec![]
            }
            KeyCode::Char('<') => {
                self.state.sidebar_width_delta -= 4;
                self.state.invalidate_patch_cache();
                vec![]
            }
            KeyCode::Char('>') => {
                self.state.sidebar_width_delta += 4;
                self.state.invalidate_patch_cache();
                vec![]
            }
            _ => vec![],
        }
    }

    fn on_mouse_event(&mut self, mouse: MouseEvent) -> Vec<GitDiffViewMessage> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                match self.state.focus {
                    PatchFocus::FileList => {
                        self.state.select_relative(-1);
                    }
                    PatchFocus::Patch => {
                        self.state.move_cursor(-3);
                    }
                    PatchFocus::CommentInput => {}
                }
                vec![]
            }
            MouseEventKind::ScrollDown => {
                match self.state.focus {
                    PatchFocus::FileList => {
                        self.state.select_relative(1);
                    }
                    PatchFocus::Patch => {
                        self.state.move_cursor(3);
                    }
                    PatchFocus::CommentInput => {}
                }
                vec![]
            }
            _ => vec![],
        }
    }

    fn navigate_down(&mut self) {
        match self.state.focus {
            PatchFocus::FileList => {
                self.state.select_relative(1);
            }
            PatchFocus::Patch => {
                self.state.move_cursor(1);
            }
            PatchFocus::CommentInput => {}
        }
    }

    fn navigate_up(&mut self) {
        match self.state.focus {
            PatchFocus::FileList => {
                self.state.select_relative(-1);
            }
            PatchFocus::Patch => {
                self.state.move_cursor(-1);
            }
            PatchFocus::CommentInput => {}
        }
    }

    fn enter_comment_mode(&mut self) {
        if self.state.focus != PatchFocus::Patch {
            return;
        }
        let cursor = self.state.cursor_line;
        if cursor >= self.state.cached_patch_line_refs.len() {
            return;
        }
        let Some(anchor) = self.state.cached_patch_line_refs[cursor] else {
            return;
        };
        self.state.draft_comment_anchor = Some(anchor);
        self.state.focus = PatchFocus::CommentInput;
        self.state.comment_buffer.clear();
        self.state.comment_cursor = 0;
        self.state.invalidate_patch_cache();
    }

    fn submit_review(&mut self) -> Vec<GitDiffViewMessage> {
        if self.state.queued_comments.is_empty() {
            return vec![];
        }
        let prompt = format_review_prompt(&self.state.queued_comments);
        vec![GitDiffViewMessage::SubmitPrompt(prompt)]
    }

    fn on_comment_input(&mut self, code: KeyCode) -> Vec<GitDiffViewMessage> {
        match code {
            KeyCode::Esc => {
                self.state.exit_comment_mode();
                vec![]
            }
            KeyCode::Enter => {
                if !self.state.comment_buffer.trim().is_empty()
                    && let Some(comment) = build_queued_comment(self.state)
                {
                    self.state.queued_comments.push(comment);
                }
                self.state.exit_comment_mode();
                vec![]
            }
            KeyCode::Char(c) => {
                let byte_pos = char_to_byte_pos(&self.state.comment_buffer, self.state.comment_cursor);
                self.state.comment_buffer.insert(byte_pos, c);
                self.state.comment_cursor += 1;
                self.state.invalidate_patch_cache();
                vec![]
            }
            KeyCode::Backspace => {
                if self.state.comment_cursor > 0 {
                    self.state.comment_cursor -= 1;
                    let byte_pos = char_to_byte_pos(&self.state.comment_buffer, self.state.comment_cursor);
                    self.state.comment_buffer.remove(byte_pos);
                    self.state.invalidate_patch_cache();
                }
                vec![]
            }
            KeyCode::Left => {
                self.state.comment_cursor = self.state.comment_cursor.saturating_sub(1);
                vec![]
            }
            KeyCode::Right => {
                let max = self.state.comment_buffer.chars().count();
                self.state.comment_cursor = (self.state.comment_cursor + 1).min(max);
                vec![]
            }
            _ => vec![],
        }
    }
}

fn render_ready(
    files: &[FileDiff],
    state: &GitDiffViewState,
    left_width: usize,
    right_width: usize,
    available_height: usize,
    context: &ViewContext,
) -> Vec<Line> {
    let theme = &context.theme;
    let selected = state.selected_file.min(files.len().saturating_sub(1));
    let selected_file = &files[selected];

    let content_height = available_height;

    let visible_entries = state.file_tree.as_ref().map(FileTree::visible_entries).unwrap_or_default();
    let tree_selected = state.file_tree.as_ref().map_or(0, FileTree::selected_visible);
    let file_scroll = state.file_list_scroll;

    let file_list_len = if visible_entries.is_empty() { files.len() } else { visible_entries.len() };
    let row_count = content_height.max(file_list_len);
    let mut rows = Vec::with_capacity(available_height);

    for i in 0..row_count {
        let mut line = Line::default();

        // Show queue indicator in last file list row
        let queue_row = !state.queued_comments.is_empty() && i == content_height.saturating_sub(1);

        if queue_row {
            let indicator = format!(
                " [{} comment{}] s:submit u:undo",
                state.queued_comments.len(),
                if state.queued_comments.len() == 1 { "" } else { "s" },
            );
            let padded = truncate_text(&indicator, left_width);
            let pad = left_width.saturating_sub(padded.chars().count());
            line.push_with_style(padded.as_ref(), Style::fg(theme.accent()).bg_color(theme.sidebar_bg()));
            if pad > 0 {
                line.push_with_style(" ".repeat(pad), Style::default().bg_color(theme.sidebar_bg()));
            }
        } else if !visible_entries.is_empty() {
            let scrolled_i = i + file_scroll;
            if let Some(entry) = visible_entries.get(scrolled_i) {
                render_file_tree_cell(&mut line, entry, scrolled_i == tree_selected, left_width, theme);
            } else {
                line.push_with_style(" ".repeat(left_width), Style::default().bg_color(theme.sidebar_bg()));
            }
        } else {
            render_file_list_cell(&mut line, files, i, selected, left_width, theme);
        }

        line.push_with_style(" ", Style::default().bg_color(theme.code_bg()));
        render_patch_cell(
            &mut line,
            selected_file,
            &state.cached_patch_lines,
            i,
            state.patch_scroll,
            state.cursor_line,
            state.focus,
            right_width,
            theme,
        );

        rows.push(line);
    }

    rows
}

#[allow(clippy::too_many_arguments)]
fn render_patch_cell(
    line: &mut Line,
    selected_file: &FileDiff,
    patch_lines: &[Line],
    row: usize,
    patch_scroll: usize,
    cursor_line: usize,
    focus: PatchFocus,
    right_width: usize,
    theme: &tui::Theme,
) {
    if row == 0 {
        let header_text = match selected_file.status {
            FileStatus::Renamed => {
                let old = selected_file.old_path.as_deref().unwrap_or("?");
                format!("{old} -> {}", selected_file.path)
            }
            _ => selected_file.path.clone(),
        };
        let status_label = match selected_file.status {
            FileStatus::Modified => "modified",
            FileStatus::Added => "new file",
            FileStatus::Deleted => "deleted",
            FileStatus::Renamed => "renamed",
            FileStatus::Untracked => "untracked",
        };
        let full_header = format!("{header_text}  ({status_label})");
        let truncated = truncate_text(&full_header, right_width);
        line.push_with_style(truncated.as_ref(), Style::default().bold());
    } else if row == 1 {
        // spacer
    } else if selected_file.binary {
        if row == 2 {
            line.push_with_style("Binary file", Style::fg(theme.text_secondary()));
        }
    } else {
        let patch_row = row - 2;
        let scrolled_row = patch_row + patch_scroll;
        if scrolled_row < patch_lines.len() {
            let is_cursor =
                matches!(focus, PatchFocus::Patch | PatchFocus::CommentInput) && scrolled_row == cursor_line;
            if is_cursor {
                append_with_cursor_highlight(line, &patch_lines[scrolled_row], theme);
            } else {
                line.append_line(&patch_lines[scrolled_row]);
            }
        }
    }
}

fn append_with_cursor_highlight(dest: &mut Line, source: &Line, theme: &tui::Theme) {
    let highlight_bg = theme.highlight_bg();
    for span in source.spans() {
        let mut style = span.style();
        style.bg = Some(highlight_bg);
        dest.push_with_style(span.text(), style);
    }
    if source.is_empty() {
        dest.push_with_style(" ", Style::default().bg_color(highlight_bg));
    }
}

fn render_message_layout(message: &str, left_width: usize, available_height: usize, theme: &tui::Theme) -> Vec<Line> {
    let mut rows = Vec::with_capacity(available_height);
    for i in 0..available_height {
        let mut line = Line::default();
        line.push_with_style(" ".repeat(left_width), Style::default().bg_color(theme.sidebar_bg()));
        line.push_with_style(" ", Style::default().bg_color(theme.code_bg()));
        if i == 0 {
            line.push_with_style(message, Style::fg(theme.text_secondary()));
        }
        rows.push(line);
    }
    rows
}

fn char_to_byte_pos(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(i, _)| i)
}

fn build_queued_comment(state: &GitDiffViewState) -> Option<QueuedComment> {
    let patch_ref = state.draft_comment_anchor?;

    let GitDiffLoadState::Ready(doc) = &state.load_state else {
        return None;
    };
    let file = doc.files.get(state.selected_file)?;
    let hunk = file.hunks.get(patch_ref.hunk_index)?;
    let patch_line = hunk.lines.get(patch_ref.line_index)?;

    let line_number = patch_line.new_line_no.or(patch_line.old_line_no);

    Some(QueuedComment {
        file_path: file.path.clone(),
        patch_ref,
        line_text: patch_line.text.clone(),
        line_number,
        line_kind: patch_line.kind,
        comment: state.comment_buffer.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::app::git_diff_mode::PatchLineRef;
    use crate::git_diff::{FileDiff, FileStatus, GitDiffDocument, Hunk, PatchLine, PatchLineKind};
    use std::path::PathBuf;
    use tui::{KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

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

    fn make_view_state(doc: GitDiffDocument) -> GitDiffViewState {
        GitDiffViewState::new(GitDiffLoadState::Ready(doc))
    }

    fn make_state_with_cache() -> GitDiffViewState {
        let mut state = make_view_state(make_test_doc());
        state.ensure_patch_cache(&ViewContext::new((100, 24)));
        state
    }

    #[test]
    fn split_patch_requires_width_109_with_default_sidebar() {
        let doc = make_test_doc();
        assert!(!should_use_split_patch(108, 0, &doc.files[0]));
        assert!(should_use_split_patch(109, 0, &doc.files[0]));
    }

    #[test]
    fn split_patch_requires_removals_even_when_wide() {
        let doc = make_test_doc();
        assert!(!should_use_split_patch(140, 0, &doc.files[1]));
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

    async fn send_key(view: &mut GitDiffView<'_>, code: KeyCode) -> Vec<GitDiffViewMessage> {
        view.on_event(&Event::Key(key(code))).await.unwrap_or_default()
    }

    async fn send_mouse(view: &mut GitDiffView<'_>, kind: MouseEventKind) -> Vec<GitDiffViewMessage> {
        view.on_event(&Event::Mouse(MouseEvent { kind, column: 0, row: 0, modifiers: KeyModifiers::NONE }))
            .await
            .unwrap_or_default()
    }

    fn has_msg(msgs: &[GitDiffViewMessage], pred: fn(&GitDiffViewMessage) -> bool) -> bool {
        msgs.iter().any(pred)
    }

    #[tokio::test]
    async fn key_emits_expected_message() {
        let cases: Vec<(KeyCode, fn(&GitDiffViewMessage) -> bool)> = vec![
            (KeyCode::Esc, |m| matches!(m, GitDiffViewMessage::Close)),
            (KeyCode::Char('r'), |m| matches!(m, GitDiffViewMessage::Refresh)),
        ];
        for (code, pred) in cases {
            let mut state = make_view_state(make_test_doc());
            let mut view = GitDiffView { state: &mut state };
            let msgs = send_key(&mut view, code).await;
            assert!(has_msg(&msgs, pred), "failed for key: {code:?}");
        }
    }

    #[tokio::test]
    async fn j_and_k_move_file_selection() {
        let mut state = make_view_state(make_test_doc());
        assert_eq!(state.selected_file, 0);

        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Char('j')).await;
        assert_eq!(view.state.selected_file, 1);

        // k from 0 wraps to last
        let mut state2 = make_view_state(make_test_doc());
        let mut view2 = GitDiffView { state: &mut state2 };
        send_key(&mut view2, KeyCode::Char('k')).await;
        assert_eq!(view2.state.selected_file, 1);
    }

    #[tokio::test]
    async fn focus_switching() {
        // Enter switches FileList -> Patch
        let mut state = make_view_state(make_test_doc());
        assert_eq!(state.focus, PatchFocus::FileList);
        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Enter).await;
        assert_eq!(view.state.focus, PatchFocus::Patch);

        // h switches Patch -> FileList
        let mut state2 = make_view_state(make_test_doc());
        state2.focus = PatchFocus::Patch;
        let mut view2 = GitDiffView { state: &mut state2 };
        send_key(&mut view2, KeyCode::Char('h')).await;
        assert_eq!(view2.state.focus, PatchFocus::FileList);
    }

    #[tokio::test]
    async fn file_selection_resets_patch_scroll() {
        let mut state = make_view_state(make_test_doc());
        state.patch_scroll = 5;
        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Char('j')).await;
        assert_eq!(view.state.patch_scroll, 0);
    }

    #[tokio::test]
    async fn c_enters_comment_mode() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 1;
        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Char('c')).await;
        assert_eq!(view.state.focus, PatchFocus::CommentInput);
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
        let mut state = make_view_state(doc);
        state.ensure_patch_cache(&ViewContext::new((100, 24)));
        state.focus = PatchFocus::Patch;
        state.cursor_line = 1; // spacer between two hunks

        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Char('c')).await;
        assert_eq!(view.state.focus, PatchFocus::Patch);
    }

    #[tokio::test]
    async fn esc_exits_comment_mode() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::CommentInput;
        state.comment_buffer = "partial".to_string();
        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Esc).await;
        assert_eq!(view.state.focus, PatchFocus::Patch);
        assert!(view.state.comment_buffer.is_empty());
    }

    #[tokio::test]
    async fn enter_queues_comment() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 1;
        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Char('c')).await;
        assert_eq!(view.state.focus, PatchFocus::CommentInput);

        for ch in "test comment".chars() {
            send_key(&mut view, KeyCode::Char(ch)).await;
        }
        assert_eq!(view.state.comment_buffer, "test comment");

        send_key(&mut view, KeyCode::Enter).await;
        assert_eq!(view.state.focus, PatchFocus::Patch);
        assert_eq!(view.state.queued_comments.len(), 1);
        assert_eq!(view.state.queued_comments[0].comment, "test comment");
        assert!(view.state.comment_buffer.is_empty());
    }

    #[tokio::test]
    async fn s_submits_review() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.queued_comments.push(queued_comment("line", "looks good", PatchLineKind::Context));
        let mut view = GitDiffView { state: &mut state };
        let msgs = send_key(&mut view, KeyCode::Char('s')).await;
        assert!(has_msg(&msgs, |m| matches!(m, GitDiffViewMessage::SubmitPrompt(_))));
        assert_eq!(
            view.state.queued_comments.len(),
            1,
            "submit should not clear queued comments before send is accepted"
        );
    }

    #[tokio::test]
    async fn s_without_comments_is_noop() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        let mut view = GitDiffView { state: &mut state };
        let msgs = send_key(&mut view, KeyCode::Char('s')).await;
        assert!(msgs.is_empty());
    }

    #[tokio::test]
    async fn u_removes_last_comment() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.queued_comments.push(queued_comment("line1", "first", PatchLineKind::Context));
        state.queued_comments.push(queued_comment("line2", "second", PatchLineKind::Added));
        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Char('u')).await;
        assert_eq!(view.state.queued_comments.len(), 1);
        assert_eq!(view.state.queued_comments[0].comment, "first");
    }

    #[test]
    fn cursor_navigation_clamps() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 0;

        state.move_cursor(-1);
        assert_eq!(state.cursor_line, 0);

        let max = state.max_patch_scroll();
        state.cursor_line = max;
        state.move_cursor(1);
        assert_eq!(state.cursor_line, max);
    }

    #[tokio::test]
    async fn cursor_replaces_scroll() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 0;
        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Char('j')).await;
        assert_eq!(view.state.cursor_line, 1);
        send_key(&mut view, KeyCode::Char('k')).await;
        assert_eq!(view.state.cursor_line, 0);
    }

    #[test]
    fn build_queued_comment_extracts_data() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::CommentInput;
        state.cursor_line = 3; // Added line "    new();"
        state.draft_comment_anchor = state.cached_patch_line_refs[3];
        state.comment_buffer = "test review".to_string();

        let comment = build_queued_comment(&state).unwrap();
        assert_eq!(comment.file_path, "a.rs");
        assert_eq!(comment.line_text, "    new();");
        assert_eq!(comment.line_kind, PatchLineKind::Added);
        assert_eq!(comment.line_number, Some(2));
        assert_eq!(comment.comment, "test review");
        assert_eq!(comment.patch_ref, PatchLineRef { hunk_index: 0, line_index: 3 });
    }

    #[tokio::test]
    async fn mouse_scroll_down_in_file_list_selects_next() {
        let mut state = make_view_state(make_test_doc());
        assert_eq!(state.selected_file, 0);
        let mut view = GitDiffView { state: &mut state };
        send_mouse(&mut view, MouseEventKind::ScrollDown).await;
        assert_eq!(view.state.selected_file, 1);
    }

    #[tokio::test]
    async fn mouse_scroll_up_in_patch_moves_cursor() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 4;
        let mut view = GitDiffView { state: &mut state };
        send_mouse(&mut view, MouseEventKind::ScrollUp).await;
        assert_eq!(view.state.cursor_line, 1);
    }

    #[tokio::test]
    async fn mouse_scroll_during_comment_input_is_noop() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::CommentInput;
        state.cursor_line = 2;
        let original_cursor = state.cursor_line;
        let original_file = state.selected_file;
        let mut view = GitDiffView { state: &mut state };
        send_mouse(&mut view, MouseEventKind::ScrollDown).await;
        assert_eq!(view.state.cursor_line, original_cursor);
        assert_eq!(view.state.selected_file, original_file);
        assert_eq!(view.state.focus, PatchFocus::CommentInput);
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

        let mut state = make_view_state(doc);
        state.ensure_patch_cache(&ViewContext::new((60, 24)));

        let offsets = state.selected_hunk_offsets();
        assert_eq!(offsets.len(), 2, "should find two hunks");

        // The second hunk offset must match where it actually appears
        // in cached_patch_line_refs, not the unwrapped line count.
        let second_hunk_ref_pos = state
            .cached_patch_line_refs
            .iter()
            .position(|r| matches!(r, Some(r) if r.hunk_index == 1))
            .expect("hunk 1 should exist in refs");
        assert_eq!(offsets[1], second_hunk_ref_pos);

        // The long line wraps, so the wrapped offset must be larger
        // than what the old unwrapped calculation would have produced (3 + 1 = 4).
        assert!(
            offsets[1] > 4,
            "second hunk offset {} should exceed unwrapped count (4) due to soft-wrapping",
            offsets[1]
        );

        // Verify jump_next_hunk lands on the correct wrapped position.
        state.focus = PatchFocus::Patch;
        state.cursor_line = 0;
        assert!(state.jump_next_hunk());
        assert_eq!(state.cursor_line, second_hunk_ref_pos);

        // And jump_prev_hunk returns to the first hunk.
        assert!(state.jump_prev_hunk());
        assert_eq!(state.cursor_line, 0);
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

    fn make_tree_state() -> GitDiffViewState {
        GitDiffViewState::new(GitDiffLoadState::Ready(make_tree_doc()))
    }

    #[tokio::test]
    async fn h_in_file_list_collapses_directory() {
        let mut state = make_tree_state();
        state.file_tree = Some(crate::components::file_tree::FileTree::from_files(&make_tree_doc().files));
        // Tree: lib/ (dir), c.rs, src/ (dir), a.rs, b.rs
        // Select "src/" dir (visible index 2)
        state.file_tree.as_mut().unwrap().navigate(2);
        let entries_before = state.file_tree.as_ref().unwrap().visible_entries().len();
        assert_eq!(entries_before, 5);

        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Char('h')).await;

        // src/ should be collapsed, hiding a.rs and b.rs
        let entries_after = view.state.file_tree.as_ref().unwrap().visible_entries().len();
        assert_eq!(entries_after, 3); // lib/, c.rs, src/ (collapsed)
    }

    #[tokio::test]
    async fn enter_on_directory_expands_it() {
        let mut state = make_tree_state();
        state.file_tree = Some(crate::components::file_tree::FileTree::from_files(&make_tree_doc().files));
        // Collapse src/ first
        state.file_tree.as_mut().unwrap().navigate(2);
        state.file_tree.as_mut().unwrap().collapse_or_parent();
        assert_eq!(state.file_tree.as_ref().unwrap().visible_entries().len(), 3);

        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Enter).await;

        // Should expand, stay in FileList
        assert_eq!(view.state.focus, PatchFocus::FileList);
        assert_eq!(view.state.file_tree.as_ref().unwrap().visible_entries().len(), 5);
    }

    #[tokio::test]
    async fn enter_on_file_switches_to_patch() {
        let mut state = make_tree_state();
        state.file_tree = Some(crate::components::file_tree::FileTree::from_files(&make_tree_doc().files));
        // Navigate to c.rs (visible index 1, which is a file)
        state.file_tree.as_mut().unwrap().navigate(1);

        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Enter).await;

        assert_eq!(view.state.focus, PatchFocus::Patch);
    }

    #[tokio::test]
    async fn enter_queues_comment_and_invalidates_cache() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 3; // Added line "    new();"
        let mut view = GitDiffView { state: &mut state };

        // Queue a comment
        send_key(&mut view, KeyCode::Char('c')).await;
        for ch in "test comment".chars() {
            send_key(&mut view, KeyCode::Char(ch)).await;
        }
        send_key(&mut view, KeyCode::Enter).await;

        assert_eq!(view.state.queued_comments.len(), 1);
        assert_eq!(view.state.queued_comments[0].comment, "test comment");

        // Cache should have been invalidated (cached lines cleared)
        assert!(view.state.cached_patch_lines.is_empty());
    }

    #[tokio::test]
    async fn undo_removes_last_comment_and_invalidates_cache() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.queued_comments.push(queued_comment("line1", "first", PatchLineKind::Context));
        state.queued_comments.push(queued_comment("line2", "second", PatchLineKind::Added));

        let mut view = GitDiffView { state: &mut state };
        send_key(&mut view, KeyCode::Char('u')).await;

        assert_eq!(view.state.queued_comments.len(), 1);
        assert_eq!(view.state.queued_comments[0].comment, "first");

        // Cache should have been invalidated (cached lines cleared)
        assert!(view.state.cached_patch_lines.is_empty());
    }

    #[test]
    fn cursor_stays_on_logical_line_after_comment_insert() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        // cursor on the Added line (visual row 3)
        state.cursor_line = 3;
        let original_ref = state.cached_patch_line_refs[3];

        // Add a comment targeting that line
        state.queued_comments.push(QueuedComment {
            file_path: "a.rs".to_string(),
            patch_ref: original_ref.unwrap(),
            line_text: "    new();".to_string(),
            line_number: Some(2),
            line_kind: PatchLineKind::Added,
            comment: "review".to_string(),
        });
        state.invalidate_patch_cache();
        state.ensure_patch_cache(&ViewContext::new((100, 24)));

        // Cursor should still be on the Added line (though its visual row may have shifted)
        let new_ref = state.cached_patch_line_refs[state.cursor_line];
        assert_eq!(new_ref, original_ref, "cursor should stay on the same logical line");
        let line_text = state.cached_patch_lines[state.cursor_line].plain_text();
        assert!(line_text.contains("new();"), "cursor should be on the added line, got: {line_text}");
    }

    #[test]
    fn cursor_on_comment_row_restores_to_anchored_line() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        // Add a comment targeting line 3 (the Added line)
        let anchor = PatchLineRef { hunk_index: 0, line_index: 3 };
        state.queued_comments.push(QueuedComment {
            file_path: "a.rs".to_string(),
            patch_ref: anchor,
            line_text: "    new();".to_string(),
            line_number: Some(2),
            line_kind: PatchLineKind::Added,
            comment: "review".to_string(),
        });
        state.invalidate_patch_cache();
        state.ensure_patch_cache(&ViewContext::new((100, 24)));

        // Find a comment row (one with None ref) and set cursor there
        let comment_row =
            state.cached_patch_line_refs.iter().position(|r| r.is_none()).expect("should have a comment row");
        state.cursor_line = comment_row;

        // Now rebuild cache again - should restore cursor to the anchored line
        state.invalidate_patch_cache();
        state.ensure_patch_cache(&ViewContext::new((100, 24)));

        // Cursor should be restored to the added line, not the comment row
        assert_eq!(
            state.cached_patch_line_refs[state.cursor_line],
            Some(anchor),
            "cursor should be restored to the anchored diff line"
        );
    }
}
