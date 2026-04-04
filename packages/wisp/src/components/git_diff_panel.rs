use crate::components::app::git_diff_mode::{PatchLineRef, QueuedComment};
use crate::components::patch_renderer::build_patch_lines;
use crate::components::split_patch_renderer::build_split_patch_lines;
use crate::git_diff::{FileDiff, FileStatus, PatchLineKind};
use tui::{Component, Cursor, Event, Frame, KeyCode, Line, MouseEventKind, Style, ViewContext};

pub struct GitDiffPanel {
    pub(crate) cached_lines: Vec<Line>,
    pub(crate) line_refs: Vec<Option<PatchLineRef>>,
    pub(crate) scroll: usize,
    pub(crate) cursor_line: usize,
    pub(crate) comment_buffer: String,
    comment_cursor: usize,
    draft_anchor: Option<PatchLineRef>,
    draft_row: Option<usize>,
    pub(crate) in_comment_mode: bool,
    file_header: String,
    file_status: FileStatus,
    binary: bool,
    saved_cursor_anchor: Option<PatchLineRef>,
    cached_for_width: Option<u16>,
}

pub enum GitDiffPanelMessage {
    CommentSubmitted { anchor: PatchLineRef, text: String },
}

impl GitDiffPanel {
    pub fn new() -> Self {
        Self {
            cached_lines: Vec::new(),
            line_refs: Vec::new(),
            scroll: 0,
            cursor_line: 0,
            comment_buffer: String::new(),
            comment_cursor: 0,
            draft_anchor: None,
            draft_row: None,
            in_comment_mode: false,
            file_header: String::new(),
            file_status: FileStatus::Modified,
            binary: false,
            saved_cursor_anchor: None,
            cached_for_width: None,
        }
    }

    pub fn invalidate_cache(&mut self) {
        self.saved_cursor_anchor = self.current_cursor_anchor();
        self.cached_for_width = None;
        self.cached_lines.clear();
        self.line_refs.clear();
        self.draft_row = None;
    }

    pub fn reset_for_new_file(&mut self) {
        self.cursor_line = 0;
        self.scroll = 0;
        self.invalidate_cache();
    }

    pub fn reset_scroll(&mut self) {
        self.scroll = 0;
    }

    pub fn is_in_comment_mode(&self) -> bool {
        self.in_comment_mode
    }

    pub fn build_draft_comment(&self, file: &FileDiff) -> Option<QueuedComment> {
        let anchor = self.draft_anchor?;
        let hunk = file.hunks.get(anchor.hunk_index)?;
        let patch_line = hunk.lines.get(anchor.line_index)?;
        let line_number = patch_line.new_line_no.or(patch_line.old_line_no);
        let comment = if self.comment_buffer.is_empty() { " ".to_string() } else { self.comment_buffer.clone() };
        Some(QueuedComment {
            file_path: file.path.clone(),
            patch_ref: anchor,
            line_text: patch_line.text.clone(),
            line_number,
            line_kind: patch_line.kind,
            comment,
        })
    }

    pub fn ensure_cache(&mut self, file: &FileDiff, comments: &[QueuedComment], width: u16) {
        if self.cached_for_width == Some(width) && !self.cached_lines.is_empty() {
            return;
        }

        let cursor_anchor = self.saved_cursor_anchor.take();
        let right_width = width as usize;

        self.update_file_header(file);

        if file.binary {
            self.cached_lines = Vec::new();
            self.line_refs = Vec::new();
        } else {
            let has_removals = file.hunks.iter().flat_map(|h| &h.lines).any(|line| line.kind == PatchLineKind::Removed);
            let use_split_patch = right_width >= 80 && has_removals;

            let ctx = ViewContext::new((width, 0));
            if use_split_patch {
                let (lines, refs) = build_split_patch_lines(file, right_width, &ctx, comments);
                self.cached_lines = lines;
                self.line_refs = refs;
            } else {
                let (lines, refs) = build_patch_lines(file, right_width, &ctx, comments);
                self.cached_lines = lines;
                self.line_refs = refs;
            }
        }
        self.cached_for_width = Some(width);

        self.restore_cursor_to_anchor(cursor_anchor);
        self.cursor_line = self.cursor_line.min(self.max_scroll());
        self.find_draft_content_row();
    }

    pub(crate) fn max_scroll(&self) -> usize {
        self.cached_lines.len().saturating_sub(1)
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

    fn ensure_cursor_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        if self.cursor_line < self.scroll {
            self.scroll = self.cursor_line;
        } else if self.cursor_line >= self.scroll + viewport_height {
            self.scroll = self.cursor_line.saturating_sub(viewport_height - 1);
        }
    }

    fn ensure_draft_visible(&mut self, viewport_height: usize) {
        if let Some(draft_row) = self.draft_row {
            let bottom = draft_row + 1;
            if bottom >= self.scroll + viewport_height {
                self.scroll = bottom.saturating_sub(viewport_height - 1);
            }
        }
    }

    fn exit_comment_mode(&mut self) {
        self.in_comment_mode = false;
        self.comment_buffer.clear();
        self.comment_cursor = 0;
        self.draft_anchor = None;
        self.invalidate_cache();
    }

    pub(crate) fn hunk_offsets(&self) -> Vec<usize> {
        let mut offsets = Vec::new();
        let mut last_hunk: Option<usize> = None;
        for (i, r) in self.line_refs.iter().enumerate() {
            if let Some(pl_ref) = r
                && last_hunk != Some(pl_ref.hunk_index)
            {
                offsets.push(i);
                last_hunk = Some(pl_ref.hunk_index);
            }
        }
        offsets
    }

    pub(crate) fn jump_next_hunk(&mut self) -> bool {
        let current = self.cursor_line;
        if let Some(&next) = self.hunk_offsets().iter().find(|&&o| o > current) {
            let next = next.min(self.max_scroll());
            let changed = next != self.cursor_line;
            self.cursor_line = next;
            return changed;
        }
        false
    }

    pub(crate) fn jump_prev_hunk(&mut self) -> bool {
        let current = self.cursor_line;
        if let Some(&prev) = self.hunk_offsets().iter().rev().find(|&&o| o < current) {
            let changed = prev != self.cursor_line;
            self.cursor_line = prev;
            return changed;
        }
        false
    }

    fn enter_comment_mode(&mut self) {
        if self.cursor_line >= self.line_refs.len() {
            return;
        }
        let Some(anchor) = self.line_refs[self.cursor_line] else {
            return;
        };
        self.draft_anchor = Some(anchor);
        self.in_comment_mode = true;
        self.comment_buffer.clear();
        self.comment_cursor = 0;
        self.invalidate_cache();
    }

    fn on_comment_input(&mut self, code: KeyCode) -> Vec<GitDiffPanelMessage> {
        match code {
            KeyCode::Esc => {
                self.exit_comment_mode();
                vec![]
            }
            KeyCode::Enter => {
                let msg = if self.comment_buffer.trim().is_empty() {
                    None
                } else {
                    self.draft_anchor.map(|anchor| GitDiffPanelMessage::CommentSubmitted {
                        anchor,
                        text: self.comment_buffer.clone(),
                    })
                };
                self.exit_comment_mode();
                msg.into_iter().collect()
            }
            KeyCode::Char(c) => {
                let byte_pos = char_to_byte_pos(&self.comment_buffer, self.comment_cursor);
                self.comment_buffer.insert(byte_pos, c);
                self.comment_cursor += 1;
                self.invalidate_cache();
                vec![]
            }
            KeyCode::Backspace => {
                if self.comment_cursor > 0 {
                    self.comment_cursor -= 1;
                    let byte_pos = char_to_byte_pos(&self.comment_buffer, self.comment_cursor);
                    self.comment_buffer.remove(byte_pos);
                    self.invalidate_cache();
                }
                vec![]
            }
            KeyCode::Left => {
                self.comment_cursor = self.comment_cursor.saturating_sub(1);
                vec![]
            }
            KeyCode::Right => {
                let max = self.comment_buffer.chars().count();
                self.comment_cursor = (self.comment_cursor + 1).min(max);
                vec![]
            }
            _ => vec![],
        }
    }

    fn current_cursor_anchor(&self) -> Option<PatchLineRef> {
        if self.cursor_line < self.line_refs.len() {
            if let Some(anchor) = self.line_refs[self.cursor_line] {
                return Some(anchor);
            }
            for i in (0..self.cursor_line).rev() {
                if let Some(anchor) = self.line_refs[i] {
                    return Some(anchor);
                }
            }
        }
        None
    }

    fn restore_cursor_to_anchor(&mut self, anchor: Option<PatchLineRef>) {
        if let Some(anchor) = anchor
            && let Some(row) = self.line_refs.iter().position(|r| *r == Some(anchor))
        {
            self.cursor_line = row;
            return;
        }
        self.cursor_line = self.cursor_line.min(self.max_scroll());
    }

    fn find_draft_content_row(&mut self) {
        self.draft_row = None;
        let Some(anchor) = self.draft_anchor else {
            return;
        };
        let Some(anchor_pos) = self.line_refs.iter().position(|r| *r == Some(anchor)) else {
            return;
        };
        let mut last_top_border = None;
        for i in (anchor_pos + 1)..self.cached_lines.len() {
            if self.line_refs.get(i).is_some_and(Option::is_some) {
                break;
            }
            let text = self.cached_lines[i].plain_text();
            if text.trim_start().starts_with('\u{250c}') {
                last_top_border = Some(i);
            }
        }
        if let Some(top) = last_top_border {
            self.draft_row = Some(top + 1);
        }
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

    fn render_patch_row(&self, line: &mut Line, row: usize, theme: &tui::Theme) {
        if row == 0 {
            let status_label = match self.file_status {
                FileStatus::Modified => "modified",
                FileStatus::Added => "new file",
                FileStatus::Deleted => "deleted",
                FileStatus::Renamed => "renamed",
                FileStatus::Untracked => "untracked",
            };
            let full_header = format!("{}  ({status_label})", self.file_header);
            line.push_with_style(&full_header, Style::default().bold());
        } else if row == 1 {
            // spacer
        } else if self.binary {
            if row == 2 {
                line.push_with_style("Binary file", Style::fg(theme.text_secondary()));
            }
        } else {
            let patch_row = row - 2;
            let scrolled_row = patch_row + self.scroll;
            if scrolled_row < self.cached_lines.len() {
                let is_cursor = scrolled_row == self.cursor_line;
                if is_cursor {
                    append_with_cursor_highlight(line, &self.cached_lines[scrolled_row], theme);
                } else {
                    line.append_line(&self.cached_lines[scrolled_row]);
                }
            }
        }
    }
}

impl Component for GitDiffPanel {
    type Message = GitDiffPanelMessage;

    async fn on_event(&mut self, event: &Event) -> Option<Vec<Self::Message>> {
        if let Event::Mouse(mouse) = event {
            return match mouse.kind {
                MouseEventKind::ScrollUp if !self.in_comment_mode => {
                    self.move_cursor(-3);
                    Some(vec![])
                }
                MouseEventKind::ScrollDown if !self.in_comment_mode => {
                    self.move_cursor(3);
                    Some(vec![])
                }
                _ => None,
            };
        }

        let Event::Key(key) = event else {
            return None;
        };

        if self.in_comment_mode {
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
        let height = ctx.size.height as usize;

        let viewport_height = height.saturating_sub(2);
        self.ensure_cursor_visible(viewport_height);
        self.ensure_draft_visible(viewport_height);

        let mut lines = Vec::with_capacity(height);
        for row in 0..height {
            let mut line = Line::default();
            self.render_patch_row(&mut line, row, theme);
            lines.push(line);
        }

        let cursor = if let Some(draft_row) = self.draft_row {
            let screen_row = draft_row.checked_sub(self.scroll).map(|r| r + 2);
            if let Some(sr) = screen_row
                && sr < height
            {
                Cursor::visible(sr, 2 + 2 + self.comment_cursor)
            } else {
                Cursor::hidden()
            }
        } else {
            Cursor::hidden()
        };

        Frame::new(lines).with_cursor(cursor)
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

fn char_to_byte_pos(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(i, _)| i)
}
