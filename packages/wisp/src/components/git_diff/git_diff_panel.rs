use super::git_diff_compositor::{GitDiffCompositor, apply_cursor_highlight};
use super::git_diff_draft_editor::{DraftCommentEdit, DraftCommentEditor};
use crate::components::app::git_diff_mode::{PatchLineRef, QueuedComment};
use crate::git_diff::{FileDiff, FileStatus, PatchLineKind};
use tui::{Component, Cursor, Event, Frame, KeyCode, Line, MouseEventKind, Style, ViewContext};

pub struct GitDiffPanel {
    pub(crate) scroll: usize,
    pub(crate) cursor_line: usize,
    file_header: String,
    file_status: FileStatus,
    binary: bool,
    saved_cursor_anchor: Option<PatchLineRef>,
    draft_editor: DraftCommentEditor,
    compositor: GitDiffCompositor,
}

pub enum GitDiffPanelMessage {
    CommentSubmitted { anchor: PatchLineRef, text: String },
}

impl GitDiffPanel {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            cursor_line: 0,
            file_header: String::new(),
            file_status: FileStatus::Modified,
            binary: false,
            saved_cursor_anchor: None,
            draft_editor: DraftCommentEditor::new(),
            compositor: GitDiffCompositor::new(),
        }
    }

    pub fn invalidate_diff_layer(&mut self) {
        self.saved_cursor_anchor = self.current_cursor_anchor();
        self.compositor.invalidate_diff_layer();
        self.compositor.invalidate_submitted_comments_layer();
    }

    pub fn invalidate_submitted_comments_layer(&mut self) {
        self.compositor.invalidate_submitted_comments_layer();
    }

    pub fn reset_for_new_file(&mut self) {
        self.cursor_line = 0;
        self.scroll = 0;
        self.draft_editor.cancel();
        self.invalidate_diff_layer();
    }

    pub fn reset_scroll(&mut self) {
        self.scroll = 0;
    }

    pub fn is_in_comment_mode(&self) -> bool {
        self.draft_editor.is_active()
    }

    pub fn ensure_layers(
        &mut self,
        file: &FileDiff,
        comments: &[&QueuedComment],
        width: u16,
        document_revision: usize,
    ) {
        let cursor_anchor = self.saved_cursor_anchor.take();
        self.update_file_header(file);

        if file.binary {
            if self.compositor.rendered_patch().is_some() {
                self.compositor.invalidate_all();
            }
            self.restore_cursor_to_anchor(cursor_anchor);
            self.cursor_line = self.cursor_line.min(self.max_scroll());
            return;
        }

        let right_width = usize::from(width);
        let has_removals =
            file.hunks.iter().flat_map(|hunk| &hunk.lines).any(|line| line.kind == PatchLineKind::Removed);
        let use_split_patch = right_width >= 80 && has_removals;

        let context = ViewContext::new((width, 0));
        self.compositor.ensure_diff_layer(file, width, use_split_patch, document_revision, &context);
        self.compositor.ensure_submitted_layer(file, comments, width, &context.theme);

        self.restore_cursor_to_anchor(cursor_anchor);
        self.cursor_line = self.cursor_line.min(self.max_scroll());
    }

    pub(crate) fn max_scroll(&self) -> usize {
        self.line_refs().len().saturating_sub(1)
    }

    pub(crate) fn move_cursor(&mut self, delta: isize) -> bool {
        let max = self.max_scroll();
        let next = if delta.is_negative() {
            self.cursor_line.saturating_sub(delta.unsigned_abs())
        } else {
            (self.cursor_line + delta.unsigned_abs()).min(max)
        };
        let changed = next != self.cursor_line;
        self.cursor_line = next;
        changed
    }

    fn move_cursor_to_start(&mut self) -> bool {
        let changed = self.cursor_line != 0;
        self.cursor_line = 0;
        changed
    }

    fn move_cursor_to_end(&mut self) -> bool {
        let next = self.max_scroll();
        let changed = next != self.cursor_line;
        self.cursor_line = next;
        changed
    }

    fn ensure_visual_row_visible(&mut self, visual_row: usize, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }

        if visual_row < self.scroll {
            self.scroll = visual_row;
        } else if visual_row >= self.scroll + viewport_height {
            self.scroll = visual_row.saturating_sub(viewport_height - 1);
        }
    }

    pub(crate) fn jump_next_hunk(&mut self) -> bool {
        let current = self.cursor_line;
        if let Some(&next) = self.hunk_offsets_slice().iter().find(|&&offset| offset > current) {
            let next = next.min(self.max_scroll());
            let changed = next != self.cursor_line;
            self.cursor_line = next;
            return changed;
        }
        false
    }

    pub(crate) fn jump_prev_hunk(&mut self) -> bool {
        let current = self.cursor_line;
        if let Some(&prev) = self.hunk_offsets_slice().iter().rev().find(|&&offset| offset < current) {
            let changed = prev != self.cursor_line;
            self.cursor_line = prev;
            return changed;
        }
        false
    }

    fn enter_comment_mode(&mut self) {
        if self.cursor_line >= self.line_refs().len() {
            return;
        }

        let Some(anchor) = self.line_refs()[self.cursor_line] else {
            return;
        };

        self.draft_editor.begin(anchor);
    }

    fn on_comment_input(&mut self, code: KeyCode) -> Vec<GitDiffPanelMessage> {
        match self.draft_editor.handle_key(code) {
            DraftCommentEdit::Submitted { anchor, text } => {
                vec![GitDiffPanelMessage::CommentSubmitted { anchor, text }]
            }
            DraftCommentEdit::Noop | DraftCommentEdit::Cancelled => vec![],
        }
    }

    fn current_cursor_anchor(&self) -> Option<PatchLineRef> {
        let line_refs = self.line_refs();

        if self.cursor_line < line_refs.len() {
            if let Some(anchor) = line_refs[self.cursor_line] {
                return Some(anchor);
            }

            for index in (0..self.cursor_line).rev() {
                if let Some(anchor) = line_refs[index] {
                    return Some(anchor);
                }
            }
        }

        None
    }

    fn restore_cursor_to_anchor(&mut self, anchor: Option<PatchLineRef>) {
        let line_refs = self.line_refs();

        if let Some(anchor) = anchor
            && let Some(row) = line_refs.iter().position(|line_ref| *line_ref == Some(anchor))
        {
            self.cursor_line = row;
            return;
        }

        self.cursor_line = self.cursor_line.min(self.max_scroll());
    }

    fn update_file_header(&mut self, file: &FileDiff) {
        self.file_header = match file.status {
            FileStatus::Renamed => {
                let old = file.old_path.as_deref().unwrap_or("?");
                format!("{old} -> {}", file.path)
            }
            _ => file.path.clone(),
        };
        self.file_status = file.status;
        self.binary = file.binary;
    }

    fn render_header_line(&self) -> Line {
        let status_label = match self.file_status {
            FileStatus::Modified => "modified",
            FileStatus::Added => "new file",
            FileStatus::Deleted => "deleted",
            FileStatus::Renamed => "renamed",
            FileStatus::Untracked => "untracked",
        };

        let mut line = Line::default();
        line.push_with_style(format!("{}  ({status_label})", self.file_header), Style::default().bold());
        line
    }

    fn render_binary_frame(&self, theme: &tui::Theme, height: usize) -> Frame {
        let mut lines = Vec::with_capacity(height);

        for row in 0..height {
            let mut line = Line::default();
            if row == 0 {
                line = self.render_header_line();
            } else if row == 2 {
                line.push_with_style("Binary file", Style::fg(theme.text_secondary()));
            }
            lines.push(line);
        }

        Frame::new(lines).with_cursor(Cursor::hidden())
    }

    fn line_refs(&self) -> &[Option<PatchLineRef>] {
        self.compositor.rendered_patch().map_or(&[], |rendered| rendered.line_refs.as_slice())
    }

    fn hunk_offsets_slice(&self) -> &[usize] {
        self.compositor.rendered_patch().map_or(&[], |rendered| rendered.hunk_offsets.as_slice())
    }
}

impl Component for GitDiffPanel {
    type Message = GitDiffPanelMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        if let Event::Mouse(mouse) = event {
            return match mouse.kind {
                MouseEventKind::ScrollUp if !self.is_in_comment_mode() => {
                    self.move_cursor(-3);
                    Some(vec![])
                }
                MouseEventKind::ScrollDown if !self.is_in_comment_mode() => {
                    self.move_cursor(3);
                    Some(vec![])
                }
                _ => None,
            };
        }

        let Event::Key(key) = event else {
            return None;
        };

        if self.is_in_comment_mode() {
            return Some(self.on_comment_input(key.code));
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_cursor(1);
                Some(vec![])
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_cursor(-1);
                Some(vec![])
            }
            KeyCode::Char('g') => {
                self.move_cursor_to_start();
                Some(vec![])
            }
            KeyCode::Char('G') => {
                self.move_cursor_to_end();
                Some(vec![])
            }
            KeyCode::PageDown => {
                self.move_cursor(20);
                Some(vec![])
            }
            KeyCode::PageUp => {
                self.move_cursor(-20);
                Some(vec![])
            }
            KeyCode::Char('n') => {
                self.jump_next_hunk();
                Some(vec![])
            }
            KeyCode::Char('p') => {
                self.jump_prev_hunk();
                Some(vec![])
            }
            KeyCode::Char('c') => {
                self.enter_comment_mode();
                Some(vec![])
            }
            _ => None,
        }
    }

    fn render(&mut self, ctx: &ViewContext) -> Frame {
        let theme = &ctx.theme;
        let height = usize::from(ctx.size.height);

        if self.binary {
            return self.render_binary_frame(theme, height);
        }

        let body_height = height.saturating_sub(2);
        let right_width = usize::from(ctx.size.width);

        let Some(rendered) = self.compositor.rendered_patch() else {
            let mut lines = vec![self.render_header_line()];
            lines.resize(height, Line::default());
            return Frame::new(lines).with_cursor(Cursor::hidden());
        };

        if rendered.lines.is_empty() {
            let mut lines = vec![self.render_header_line()];
            lines.resize(height, Line::default());
            return Frame::new(lines).with_cursor(Cursor::hidden());
        }

        let diff_lines: Vec<Line> = rendered
            .lines
            .iter()
            .enumerate()
            .map(|(i, line)| if i == self.cursor_line { apply_cursor_highlight(line, theme) } else { line.clone() })
            .collect();
        let mut frame = Frame::new(diff_lines);

        let mut cursor_offset: usize = 0;
        for splice in self.compositor.comment_splices().iter().rev() {
            if splice.after_row < self.cursor_line {
                cursor_offset += splice.frame.lines().len();
            }
            frame = frame.splice(splice.after_row, splice.frame.clone());
        }

        let draft_state = self.draft_editor.state();
        let mut draft_end_visual_row = None;
        if let Some(ref draft) = draft_state
            && let Some(draft_splice) = self.compositor.draft_splice(draft, right_width, theme)
        {
            if draft_splice.splice.after_row < self.cursor_line {
                cursor_offset += draft_splice.splice.frame.lines().len();
            }

            let submitted_offset: usize = self
                .compositor
                .comment_splices()
                .iter()
                .filter(|s| s.after_row <= draft_splice.splice.after_row)
                .map(|s| s.frame.lines().len())
                .sum();

            let draft_frame_height = draft_splice.splice.frame.lines().len();
            frame = frame.splice(draft_splice.splice.after_row + submitted_offset, draft_splice.splice.frame);

            draft_end_visual_row = Some(draft_splice.splice.after_row + submitted_offset + draft_frame_height);
        }

        let cursor_visual_row = self.cursor_line + cursor_offset;
        let max_scroll = frame.lines().len().saturating_sub(body_height);
        self.scroll = self.scroll.min(max_scroll);
        self.ensure_visual_row_visible(cursor_visual_row, body_height);

        if body_height > 0
            && let Some(splice) = self.compositor.comment_splices().iter().find(|s| s.after_row == self.cursor_line)
        {
            let comment_end = cursor_visual_row + splice.frame.lines().len();
            if comment_end >= self.scroll + body_height {
                self.scroll = comment_end.saturating_sub(body_height - 1).min(cursor_visual_row);
            }
        }

        if let Some(draft_end) = draft_end_visual_row {
            self.ensure_visual_row_visible(draft_end, body_height);
        }
        self.scroll = self.scroll.min(max_scroll);

        let viewport = frame.scroll(self.scroll, body_height);

        let mut header_lines = vec![self.render_header_line()];
        if height > 1 {
            header_lines.push(Line::default());
        }

        Frame::vstack([Frame::new(header_lines), viewport])
    }
}
