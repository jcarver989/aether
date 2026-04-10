use crate::components::app::git_diff_mode::{PatchLineRef, QueuedComment};
use std::collections::HashMap;
use tui::{Color, Line, Style, Theme};

pub struct CommentBlock {
    pub anchor: PatchLineRef,
    pub rows: Vec<Line>,
    pub content_row_offset: usize,
}

#[derive(Clone)]
pub struct DraftCommentState {
    pub anchor: PatchLineRef,
    pub text: String,
    pub cursor_position: usize,
}

pub struct DraftCommentBlock {
    pub block: CommentBlock,
    pub cursor_row_offset: usize,
    pub cursor_col: usize,
}

pub fn render_comment_blocks(comments: &[&QueuedComment], width: usize, theme: &Theme) -> Vec<CommentBlock> {
    let mut grouped: Vec<(PatchLineRef, Vec<&QueuedComment>)> = Vec::new();
    let mut index_by_anchor: HashMap<PatchLineRef, usize> = HashMap::new();

    for comment in comments {
        if let Some(index) = index_by_anchor.get(&comment.patch_ref).copied() {
            grouped[index].1.push(comment);
        } else {
            let index = grouped.len();
            grouped.push((comment.patch_ref, vec![comment]));
            index_by_anchor.insert(comment.patch_ref, index);
        }
    }

    grouped
        .into_iter()
        .map(|(anchor, grouped_comments)| render_grouped_comment_block(anchor, &grouped_comments, width, theme))
        .collect()
}

pub fn render_comment_block(anchor: PatchLineRef, text: &str, width: usize, theme: &Theme) -> CommentBlock {
    let (rows, content_row_offset) = render_comment_box_rows(text, width, theme);
    CommentBlock { anchor, rows, content_row_offset }
}

pub fn render_draft_comment_block(draft: &DraftCommentState, width: usize, theme: &Theme) -> DraftCommentBlock {
    let text = if draft.text.is_empty() { " " } else { &draft.text };
    let (rows, content_row_offset) = render_comment_box_rows(text, width, theme);

    let inner_width = width.saturating_sub(draft_text_col_start());
    let (cursor_row, cursor_col) = cursor_row_col(text, draft.cursor_position, inner_width);

    DraftCommentBlock {
        block: CommentBlock { anchor: draft.anchor, rows, content_row_offset },
        cursor_row_offset: content_row_offset + cursor_row,
        cursor_col: draft_text_col_start() + cursor_col,
    }
}

fn render_grouped_comment_block(
    anchor: PatchLineRef,
    comments: &[&QueuedComment],
    width: usize,
    theme: &Theme,
) -> CommentBlock {
    let mut rows = Vec::new();
    let mut content_row_offset = 1;

    for (index, comment) in comments.iter().enumerate() {
        let comment_block = render_comment_block(anchor, &comment.comment, width, theme);
        if index == 0 {
            content_row_offset = comment_block.content_row_offset;
        }
        rows.extend(comment_block.rows);
    }

    CommentBlock { anchor, rows, content_row_offset }
}

fn render_comment_box_rows(text: &str, width: usize, theme: &Theme) -> (Vec<Line>, usize) {
    let indent = 2;
    let box_left = "│ ";
    let bg = theme.sidebar_bg();
    let border_fg = theme.muted();
    let text_fg = theme.text_primary();

    let dashes = width.saturating_sub(indent + 1);
    let inner_width = width.saturating_sub(indent + box_left.len() + 1);
    let wrapped = wrap_text(text, inner_width);

    let mut rows = Vec::new();
    push_border_row(&mut rows, "┌", indent, dashes, width, border_fg, bg);

    for text_line in &wrapped {
        let mut row = Line::default();
        row.push_with_style(" ".repeat(indent), Style::default().bg_color(bg));
        row.push_with_style(box_left, Style::fg(border_fg).bg_color(bg));
        row.push_with_style(text_line.as_str(), Style::fg(text_fg).bg_color(bg));
        row.extend_bg_to_width(width);
        rows.push(row);
    }

    push_border_row(&mut rows, "└", indent, dashes, width, border_fg, bg);
    (rows, 1)
}

fn push_border_row(
    rows: &mut Vec<Line>,
    corner: &str,
    indent: usize,
    dashes: usize,
    width: usize,
    border_fg: Color,
    bg: Color,
) {
    let mut row = Line::default();
    row.push_with_style(" ".repeat(indent), Style::default().bg_color(bg));
    row.push_with_style(corner, Style::fg(border_fg).bg_color(bg));
    row.push_with_style("─".repeat(dashes), Style::fg(border_fg).bg_color(bg));
    row.extend_bg_to_width(width);
    rows.push(row);
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if current_len == 0 {
            current.push_str(word);
            current_len = word_len;
        } else if current_len + 1 + word_len <= max_width {
            current.push(' ');
            current.push_str(word);
            current_len += 1 + word_len;
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
            current_len = word_len;
        }
    }

    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }

    lines
}

fn cursor_row_col(text: &str, cursor_position: usize, max_width: usize) -> (usize, usize) {
    if max_width == 0 {
        return (0, 0);
    }

    let canonical = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let wrapped = wrap_text(if canonical.is_empty() { " " } else { &canonical }, max_width);
    let cursor = cursor_position.min(canonical.chars().count());

    let mut consumed = 0usize;
    for (row_idx, line) in wrapped.iter().enumerate() {
        let line_len = line.chars().count();
        if cursor <= consumed + line_len {
            return (row_idx, cursor.saturating_sub(consumed));
        }
        consumed += line_len + 1;
    }

    wrapped.last().map_or((0, 0), |last| (wrapped.len().saturating_sub(1), last.chars().count()))
}

fn draft_text_col_start() -> usize {
    2 + 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queued(anchor: PatchLineRef, comment: &str) -> QueuedComment {
        QueuedComment {
            file_path: "test.rs".to_string(),
            patch_ref: anchor,
            line_text: "line".to_string(),
            line_number: Some(1),
            line_kind: crate::git_diff::PatchLineKind::Added,
            comment: comment.to_string(),
        }
    }

    #[test]
    fn wrap_text_basic() {
        let result = wrap_text("hello world foo bar", 10);
        assert_eq!(result, vec!["hello", "world foo", "bar"]);
    }

    #[test]
    fn render_comment_blocks_preserves_anchor_order() {
        let anchor = PatchLineRef { hunk_index: 0, line_index: 1 };
        let first = queued(anchor, "alpha");
        let second = queued(anchor, "beta");
        let refs = vec![&first, &second];
        let theme = tui::ViewContext::new((80, 24)).theme;

        let blocks = render_comment_blocks(&refs, 60, &theme);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].rows.iter().any(|row| row.plain_text().contains("alpha")));
        assert!(blocks[0].rows.iter().any(|row| row.plain_text().contains("beta")));
    }

    #[test]
    fn draft_block_has_borders_and_cursor() {
        let draft = DraftCommentState {
            anchor: PatchLineRef { hunk_index: 0, line_index: 1 },
            text: "test comment".to_string(),
            cursor_position: 4,
        };
        let theme = tui::ViewContext::new((80, 24)).theme;
        let rendered = render_draft_comment_block(&draft, 60, &theme);

        assert!(rendered.block.rows.len() >= 3);
        assert!(rendered.block.rows[0].plain_text().contains('┌'));
        assert!(rendered.block.rows.last().is_some_and(|row| row.plain_text().contains('└')));
        assert!(rendered.cursor_col >= draft_text_col_start());
    }
}
