use crate::components::app::git_diff_mode::{QueuedComment, format_review_prompt};
use crate::components::app::{GitDiffLoadState, GitDiffViewState, PatchFocus};
use crate::components::file_list_renderer::render_file_list_cell;
pub use crate::components::patch_renderer::build_patch_lines;
use crate::git_diff::{FileDiff, FileStatus, PatchLineKind};
use crate::tui::{Component, Event, Frame, KeyCode, Line, Style, ViewContext, truncate_text};

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

fn render_git_diff_state(state: &GitDiffViewState, context: &ViewContext) -> Vec<Line> {
    let theme = &context.theme;
    let total_width = context.size.width as usize;
    if total_width < 10 {
        return vec![Line::new("Too narrow")];
    }

    let left_width = (total_width / 3)
        .clamp(20, 28)
        .min(total_width.saturating_sub(4));
    let right_width = total_width.saturating_sub(left_width + 1);
    let available_height = context.size.height as usize;

    match &state.load_state {
        GitDiffLoadState::Loading => {
            render_message_layout("Loading...", left_width, available_height, theme)
        }
        GitDiffLoadState::Empty => render_message_layout(
            "No changes in working tree relative to HEAD",
            left_width,
            available_height,
            theme,
        ),
        GitDiffLoadState::Error { message } => {
            let msg = format!("Git diff unavailable: {message}");
            render_message_layout(&msg, left_width, available_height, theme)
        }
        GitDiffLoadState::Ready(doc) if doc.files.is_empty() => render_message_layout(
            "No changes in working tree relative to HEAD",
            left_width,
            available_height,
            theme,
        ),
        GitDiffLoadState::Ready(doc) => render_ready(
            &doc.files,
            state,
            left_width,
            right_width,
            available_height,
            context,
        ),
    }
}

impl Component for GitDiffView<'_> {
    type Message = GitDiffViewMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        let Event::Key(key) = event else {
            return None;
        };

        if self.state.focus == PatchFocus::CommentInput {
            return Some(self.on_comment_input(key.code));
        }

        match key.code {
            KeyCode::Esc => Some(vec![GitDiffViewMessage::Close]),
            KeyCode::Char('r') => Some(vec![GitDiffViewMessage::Refresh]),
            KeyCode::Char('h') | KeyCode::Left => {
                self.state.set_focus(PatchFocus::FileList);
                Some(vec![])
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                self.state.set_focus(PatchFocus::Patch);
                Some(vec![])
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.navigate_down();
                Some(vec![])
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.navigate_up();
                Some(vec![])
            }
            KeyCode::Char('g') => {
                self.state.move_cursor_to_start();
                Some(vec![])
            }
            KeyCode::Char('G') => {
                self.state.move_cursor_to_end();
                Some(vec![])
            }
            KeyCode::PageDown => {
                self.state.move_cursor(20);
                Some(vec![])
            }
            KeyCode::PageUp => {
                self.state.move_cursor(-20);
                Some(vec![])
            }
            KeyCode::Char('n') => {
                self.state.jump_next_hunk();
                Some(vec![])
            }
            KeyCode::Char('p') => {
                self.state.jump_prev_hunk();
                Some(vec![])
            }
            KeyCode::Char('c') => {
                self.enter_comment_mode();
                Some(vec![])
            }
            KeyCode::Char('s') => Some(self.submit_review()),
            KeyCode::Char('u') => {
                self.state.queued_comments.pop();
                Some(vec![])
            }
            _ => Some(vec![]),
        }
    }

    fn render(&self, context: &ViewContext) -> Frame {
        Frame::new(render_git_diff_state(self.state, context))
    }
}

impl GitDiffView<'_> {
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
        if self.state.cached_patch_line_refs[cursor].is_none() {
            return;
        }
        self.state.focus = PatchFocus::CommentInput;
        self.state.comment_buffer.clear();
        self.state.comment_cursor = 0;
    }

    fn submit_review(&mut self) -> Vec<GitDiffViewMessage> {
        if self.state.queued_comments.is_empty() {
            return vec![];
        }
        let comments = std::mem::take(&mut self.state.queued_comments);
        let prompt = format_review_prompt(&comments);
        vec![GitDiffViewMessage::SubmitPrompt(prompt)]
    }

    fn on_comment_input(&mut self, code: KeyCode) -> Vec<GitDiffViewMessage> {
        match code {
            KeyCode::Esc => {
                self.state.focus = PatchFocus::Patch;
                self.state.comment_buffer.clear();
                self.state.comment_cursor = 0;
                vec![]
            }
            KeyCode::Enter => {
                if let Some(comment) = build_queued_comment(self.state) {
                    self.state.queued_comments.push(comment);
                }
                self.state.focus = PatchFocus::Patch;
                self.state.comment_buffer.clear();
                self.state.comment_cursor = 0;
                vec![]
            }
            KeyCode::Char(c) => {
                let byte_pos =
                    char_to_byte_pos(&self.state.comment_buffer, self.state.comment_cursor);
                self.state.comment_buffer.insert(byte_pos, c);
                self.state.comment_cursor += 1;
                vec![]
            }
            KeyCode::Backspace => {
                if self.state.comment_cursor > 0 {
                    self.state.comment_cursor -= 1;
                    let byte_pos =
                        char_to_byte_pos(&self.state.comment_buffer, self.state.comment_cursor);
                    self.state.comment_buffer.remove(byte_pos);
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

    let show_comment_bar = state.focus == PatchFocus::CommentInput;
    let content_height = if show_comment_bar {
        available_height.saturating_sub(1)
    } else {
        available_height
    };

    let row_count = content_height.max(files.len());
    let mut rows = Vec::with_capacity(available_height);

    for i in 0..row_count {
        let mut line = Line::default();

        // Show queue indicator in last file list row
        let queue_row = !state.queued_comments.is_empty() && i == content_height.saturating_sub(1);

        if queue_row {
            let indicator = format!(
                " [{} comment{}] s:submit u:undo",
                state.queued_comments.len(),
                if state.queued_comments.len() == 1 {
                    ""
                } else {
                    "s"
                },
            );
            let padded = truncate_text(&indicator, left_width);
            let pad = left_width.saturating_sub(padded.chars().count());
            line.push_with_style(padded.as_ref(), Style::fg(theme.accent()));
            if pad > 0 {
                line.push_text(" ".repeat(pad));
            }
        } else {
            render_file_list_cell(&mut line, files, i, selected, left_width, theme);
        }

        line.push_with_style("\u{2502}", Style::fg(theme.text_secondary()));
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

    if show_comment_bar {
        let mut bar = Line::default();
        let label = format!("Comment: {}", state.comment_buffer);
        let truncated = truncate_text(&label, left_width + 1 + right_width);
        bar.push_with_style(
            truncated.as_ref(),
            Style::fg(theme.text_primary()).bg_color(theme.highlight_bg()),
        );
        let bar_width = truncated.chars().count();
        let total = left_width + 1 + right_width;
        if bar_width < total {
            bar.push_with_style(
                " ".repeat(total - bar_width),
                Style::default().bg_color(theme.highlight_bg()),
            );
        }
        rows.push(bar);
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
    theme: &crate::tui::Theme,
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
            let is_cursor = matches!(focus, PatchFocus::Patch | PatchFocus::CommentInput)
                && scrolled_row == cursor_line;
            if is_cursor {
                append_with_cursor_highlight(line, &patch_lines[scrolled_row], theme);
            } else {
                line.append_line(&patch_lines[scrolled_row]);
            }
        }
    }
}

fn append_with_cursor_highlight(dest: &mut Line, source: &Line, theme: &crate::tui::Theme) {
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

fn render_message_layout(
    message: &str,
    left_width: usize,
    available_height: usize,
    theme: &crate::tui::Theme,
) -> Vec<Line> {
    let mut rows = Vec::with_capacity(available_height);
    for i in 0..available_height {
        let mut line = Line::default();
        line.push_text(" ".repeat(left_width));
        line.push_with_style("\u{2502}", Style::fg(theme.text_secondary()));
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
    let cursor = state.cursor_line;
    let patch_ref = state.cached_patch_line_refs.get(cursor)?.as_ref()?;

    let GitDiffLoadState::Ready(doc) = &state.load_state else {
        return None;
    };
    let file = doc.files.get(state.selected_file)?;
    let hunk = file.hunks.get(patch_ref.hunk_index)?;
    let patch_line = hunk.lines.get(patch_ref.line_index)?;

    // Reconstruct hunk text as unified diff
    let mut hunk_text = String::new();
    for pl in &hunk.lines {
        match pl.kind {
            PatchLineKind::Context => {
                hunk_text.push(' ');
                hunk_text.push_str(&pl.text);
                hunk_text.push('\n');
            }
            PatchLineKind::Added => {
                hunk_text.push('+');
                hunk_text.push_str(&pl.text);
                hunk_text.push('\n');
            }
            PatchLineKind::Removed => {
                hunk_text.push('-');
                hunk_text.push_str(&pl.text);
                hunk_text.push('\n');
            }
            PatchLineKind::HunkHeader | PatchLineKind::Meta => {
                hunk_text.push_str(&pl.text);
                hunk_text.push('\n');
            }
        }
    }
    // Trim trailing newline
    if hunk_text.ends_with('\n') {
        hunk_text.pop();
    }

    let line_number = patch_line.new_line_no.or(patch_line.old_line_no);

    Some(QueuedComment {
        file_path: file.path.clone(),
        hunk_index: patch_ref.hunk_index,
        hunk_text,
        line_text: patch_line.text.clone(),
        line_number,
        line_kind: patch_line.kind,
        comment: state.comment_buffer.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_diff::{FileDiff, FileStatus, GitDiffDocument, Hunk, PatchLine, PatchLineKind};
    use crate::tui::{KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn make_test_doc() -> GitDiffDocument {
        GitDiffDocument {
            repo_root: PathBuf::from("/tmp/test"),
            files: vec![
                FileDiff {
                    old_path: Some("a.rs".to_string()),
                    path: "a.rs".to_string(),
                    status: FileStatus::Modified,
                    hunks: vec![Hunk {
                        header: "@@ -1,3 +1,3 @@".to_string(),
                        old_start: 1,
                        old_count: 3,
                        new_start: 1,
                        new_count: 3,
                        lines: vec![
                            PatchLine {
                                kind: PatchLineKind::HunkHeader,
                                text: "@@ -1,3 +1,3 @@".to_string(),
                                old_line_no: None,
                                new_line_no: None,
                            },
                            PatchLine {
                                kind: PatchLineKind::Context,
                                text: "fn main() {".to_string(),
                                old_line_no: Some(1),
                                new_line_no: Some(1),
                            },
                            PatchLine {
                                kind: PatchLineKind::Removed,
                                text: "    old();".to_string(),
                                old_line_no: Some(2),
                                new_line_no: None,
                            },
                            PatchLine {
                                kind: PatchLineKind::Added,
                                text: "    new();".to_string(),
                                old_line_no: None,
                                new_line_no: Some(2),
                            },
                            PatchLine {
                                kind: PatchLineKind::Context,
                                text: "}".to_string(),
                                old_line_no: Some(3),
                                new_line_no: Some(3),
                            },
                        ],
                    }],
                    binary: false,
                },
                FileDiff {
                    old_path: None,
                    path: "b.rs".to_string(),
                    status: FileStatus::Added,
                    hunks: vec![Hunk {
                        header: "@@ -0,0 +1,1 @@".to_string(),
                        old_start: 0,
                        old_count: 0,
                        new_start: 1,
                        new_count: 1,
                        lines: vec![
                            PatchLine {
                                kind: PatchLineKind::HunkHeader,
                                text: "@@ -0,0 +1,1 @@".to_string(),
                                old_line_no: None,
                                new_line_no: None,
                            },
                            PatchLine {
                                kind: PatchLineKind::Added,
                                text: "new_content".to_string(),
                                old_line_no: None,
                                new_line_no: Some(1),
                            },
                        ],
                    }],
                    binary: false,
                },
            ],
        }
    }

    fn make_view_state(doc: GitDiffDocument) -> GitDiffViewState {
        GitDiffViewState::new(GitDiffLoadState::Ready(doc))
    }

    #[test]
    fn esc_emits_close() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        let mut view = GitDiffView { state: &mut state };
        let result = view.on_event(&Event::Key(key(KeyCode::Esc)));
        assert!(
            result
                .unwrap_or_default()
                .iter()
                .any(|m| matches!(m, GitDiffViewMessage::Close))
        );
    }

    #[test]
    fn r_emits_refresh() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        let mut view = GitDiffView { state: &mut state };
        let result = view.on_event(&Event::Key(key(KeyCode::Char('r'))));
        assert!(
            result
                .unwrap_or_default()
                .iter()
                .any(|m| matches!(m, GitDiffViewMessage::Refresh))
        );
    }

    #[test]
    fn j_moves_file_selection_down() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        assert_eq!(state.selected_file, 0);

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Char('j'))));
        assert_eq!(view.state.selected_file, 1);
    }

    #[test]
    fn k_moves_file_selection_up_with_wrap() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        assert_eq!(state.selected_file, 0);

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Char('k'))));
        assert_eq!(view.state.selected_file, 1); // wraps from 0 to last
    }

    #[test]
    fn enter_switches_to_patch_focus() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        assert_eq!(state.focus, PatchFocus::FileList);

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Enter)));
        assert_eq!(view.state.focus, PatchFocus::Patch);
    }

    #[test]
    fn h_switches_to_file_list_focus() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        state.focus = PatchFocus::Patch;

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Char('h'))));
        assert_eq!(view.state.focus, PatchFocus::FileList);
    }

    #[test]
    fn file_selection_resets_patch_scroll() {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        state.patch_scroll = 5;

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Char('j'))));
        assert_eq!(view.state.patch_scroll, 0);
    }

    fn make_state_with_cache() -> GitDiffViewState {
        let doc = make_test_doc();
        let mut state = make_view_state(doc);
        let context = ViewContext::new((100, 24));
        state.ensure_patch_cache(&context);
        state
    }

    #[test]
    fn c_enters_comment_mode() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 1; // Context line (has a ref)

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Char('c'))));
        assert_eq!(view.state.focus, PatchFocus::CommentInput);
    }

    #[test]
    fn c_on_spacer_is_noop() {
        let doc = GitDiffDocument {
            repo_root: PathBuf::from("/tmp/test"),
            files: vec![FileDiff {
                old_path: None,
                path: "a.rs".to_string(),
                status: FileStatus::Modified,
                hunks: vec![
                    Hunk {
                        header: "@@ -1,1 +1,1 @@".to_string(),
                        old_start: 1,
                        old_count: 1,
                        new_start: 1,
                        new_count: 1,
                        lines: vec![PatchLine {
                            kind: PatchLineKind::HunkHeader,
                            text: "@@ -1,1 +1,1 @@".to_string(),
                            old_line_no: None,
                            new_line_no: None,
                        }],
                    },
                    Hunk {
                        header: "@@ -5,1 +5,1 @@".to_string(),
                        old_start: 5,
                        old_count: 1,
                        new_start: 5,
                        new_count: 1,
                        lines: vec![PatchLine {
                            kind: PatchLineKind::HunkHeader,
                            text: "@@ -5,1 +5,1 @@".to_string(),
                            old_line_no: None,
                            new_line_no: None,
                        }],
                    },
                ],
                binary: false,
            }],
        };
        let mut state = make_view_state(doc);
        let context = ViewContext::new((100, 24));
        state.ensure_patch_cache(&context);
        state.focus = PatchFocus::Patch;
        // The spacer line between hunks has None ref
        state.cursor_line = 1; // spacer between two hunks

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Char('c'))));
        assert_eq!(view.state.focus, PatchFocus::Patch);
    }

    #[test]
    fn esc_exits_comment_mode() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::CommentInput;
        state.comment_buffer = "partial".to_string();

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Esc)));
        assert_eq!(view.state.focus, PatchFocus::Patch);
        assert!(view.state.comment_buffer.is_empty());
    }

    #[test]
    fn enter_queues_comment() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 1; // Context line
        // Enter comment mode
        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Char('c'))));
        assert_eq!(view.state.focus, PatchFocus::CommentInput);

        // Type some text
        for ch in "test comment".chars() {
            view.on_event(&Event::Key(key(KeyCode::Char(ch))));
        }
        assert_eq!(view.state.comment_buffer, "test comment");

        // Submit with Enter
        view.on_event(&Event::Key(key(KeyCode::Enter)));
        assert_eq!(view.state.focus, PatchFocus::Patch);
        assert_eq!(view.state.queued_comments.len(), 1);
        assert_eq!(view.state.queued_comments[0].comment, "test comment");
        assert!(view.state.comment_buffer.is_empty());
    }

    #[test]
    fn s_submits_review() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.queued_comments.push(QueuedComment {
            file_path: "a.rs".to_string(),
            hunk_index: 0,
            hunk_text: "hunk".to_string(),
            line_text: "line".to_string(),
            line_number: Some(1),
            line_kind: PatchLineKind::Context,
            comment: "looks good".to_string(),
        });

        let mut view = GitDiffView { state: &mut state };
        let result = view.on_event(&Event::Key(key(KeyCode::Char('s'))));
        assert!(
            result
                .unwrap_or_default()
                .iter()
                .any(|m| matches!(m, GitDiffViewMessage::SubmitPrompt(_)))
        );
    }

    #[test]
    fn s_without_comments_is_noop() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;

        let mut view = GitDiffView { state: &mut state };
        let result = view.on_event(&Event::Key(key(KeyCode::Char('s'))));
        assert!(result.unwrap_or_default().is_empty());
    }

    #[test]
    fn u_removes_last_comment() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.queued_comments.push(QueuedComment {
            file_path: "a.rs".to_string(),
            hunk_index: 0,
            hunk_text: "hunk".to_string(),
            line_text: "line1".to_string(),
            line_number: Some(1),
            line_kind: PatchLineKind::Context,
            comment: "first".to_string(),
        });
        state.queued_comments.push(QueuedComment {
            file_path: "a.rs".to_string(),
            hunk_index: 0,
            hunk_text: "hunk".to_string(),
            line_text: "line2".to_string(),
            line_number: Some(2),
            line_kind: PatchLineKind::Added,
            comment: "second".to_string(),
        });

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Char('u'))));
        assert_eq!(view.state.queued_comments.len(), 1);
        assert_eq!(view.state.queued_comments[0].comment, "first");
    }

    #[test]
    fn cursor_navigation_clamps() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 0;

        // k at 0 stays at 0
        state.move_cursor(-1);
        assert_eq!(state.cursor_line, 0);

        // Move to end
        let max = state.max_patch_scroll();
        state.cursor_line = max;
        state.move_cursor(1);
        assert_eq!(state.cursor_line, max);
    }

    #[test]
    fn cursor_replaces_scroll() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 0;

        let mut view = GitDiffView { state: &mut state };
        view.on_event(&Event::Key(key(KeyCode::Char('j'))));
        assert_eq!(view.state.cursor_line, 1);

        view.on_event(&Event::Key(key(KeyCode::Char('k'))));
        assert_eq!(view.state.cursor_line, 0);
    }

    #[test]
    fn build_queued_comment_extracts_data() {
        let mut state = make_state_with_cache();
        state.focus = PatchFocus::Patch;
        state.cursor_line = 3; // Added line "    new();"
        state.comment_buffer = "test review".to_string();

        let comment = build_queued_comment(&state).unwrap();
        assert_eq!(comment.file_path, "a.rs");
        assert_eq!(comment.hunk_index, 0);
        assert_eq!(comment.line_text, "    new();");
        assert_eq!(comment.line_kind, PatchLineKind::Added);
        assert_eq!(comment.line_number, Some(2));
        assert_eq!(comment.comment, "test review");
        assert!(comment.hunk_text.contains("+    new();"));
        assert!(comment.hunk_text.contains("-    old();"));
    }
}
