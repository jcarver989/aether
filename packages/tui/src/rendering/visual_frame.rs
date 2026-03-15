use crate::Frame;
use crate::rendering::render_context::Size;

use super::frame::Cursor;
use super::line::Line;
use super::soft_wrap::soft_wrap_lines_with_map;

/// Result of diffing two VisualFrames' visible lines.
#[derive(Debug)]
pub struct LineDiff<'a> {
    /// Row index to start rewriting from.
    pub rewrite_from: usize,
    /// Lines to write starting at `rewrite_from`.
    pub lines: &'a [Line],
    /// Number of visible rows in the previous frame.
    pub previous_row_count: usize,
}

/// Terminal-ready visual frame prepared for the terminal.
///
/// This is the pure mapping step from logical output (`Frame`) to terminal-ready
/// visual state. It is responsible for:
/// - clamping cursor to logical content
/// - applying final soft-wrap using terminal width
/// - remapping logical cursor position to visual row/col after wrapping
/// - splitting prepared output into scrollback vs visible lines
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualFrame {
    scrollback_lines: Vec<Line>,
    visible_lines: Vec<Line>,
    cursor: Cursor,
    overflow: usize,
}

impl VisualFrame {
    /// Creates a `VisualFrame` from a logical Frame, applying soft-wrap and viewport split.
    pub fn from_frame(frame: &Frame, size: Size, flushed_visual_count: usize) -> Self {
        let (wrapped_lines, logical_to_visual) =
            soft_wrap_lines_with_map(frame.lines(), size.width);

        let mut visual_cursor_row = logical_to_visual
            .get(frame.cursor().row)
            .copied()
            .unwrap_or_else(|| wrapped_lines.len().saturating_sub(1));

        let mut visual_cursor_col = frame.cursor().col;
        let width = usize::from(size.width);
        if width > 0 {
            visual_cursor_row += visual_cursor_col / width;
            visual_cursor_col %= width;
        } else {
            visual_cursor_col = 0;
        }

        if visual_cursor_row >= wrapped_lines.len() {
            visual_cursor_row = wrapped_lines.len().saturating_sub(1);
        }

        let viewport_rows = usize::from(size.height.max(1));
        let total_lines = wrapped_lines.len();
        let overflow = total_lines.saturating_sub(viewport_rows);
        let cursor_row_after_overflow = visual_cursor_row.saturating_sub(overflow);

        let scrollback_lines = if overflow > flushed_visual_count {
            wrapped_lines[flushed_visual_count..overflow].to_vec()
        } else {
            Vec::new()
        };

        let visible_lines = wrapped_lines[overflow..].to_vec();
        let final_cursor_row = if visual_cursor_row >= overflow {
            cursor_row_after_overflow.min(visible_lines.len().saturating_sub(1))
        } else {
            0
        };

        Self {
            scrollback_lines,
            visible_lines,
            cursor: Cursor {
                row: final_cursor_row,
                col: visual_cursor_col,
                is_visible: frame.cursor().is_visible,
            },
            overflow,
        }
    }

    pub fn scrollback_lines(&self) -> &[Line] {
        &self.scrollback_lines
    }

    pub fn visible_lines(&self) -> &[Line] {
        &self.visible_lines
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn overflow(&self) -> usize {
        self.overflow
    }

    /// Create an empty VisualFrame with no lines and no overflow.
    pub fn empty() -> Self {
        Self {
            scrollback_lines: Vec::new(),
            visible_lines: Vec::new(),
            cursor: Cursor::hidden(),
            overflow: 0,
        }
    }

    /// Diff this frame's visible lines against `new`, returning the minimal rewrite needed.
    /// Returns `None` if visible lines are identical.
    pub fn diff<'a>(&self, new: &'a VisualFrame) -> Option<LineDiff<'a>> {
        let prev = &self.visible_lines;
        let next = &new.visible_lines;
        if next == prev {
            return None;
        }

        let first_diff = prev
            .iter()
            .zip(next.iter())
            .position(|(old, new)| old != new)
            .unwrap_or(prev.len().min(next.len()));

        let rewrite_from = if next.is_empty() {
            0
        } else {
            first_diff.min(next.len() - 1)
        };

        Some(LineDiff {
            rewrite_from,
            lines: &next[rewrite_from..],
            previous_row_count: prev.len(),
        })
    }
}

/// Prepare logical lines for scrollback using the same width semantics as `VisualFrame`.
pub fn prepare_lines_for_scrollback(lines: &[Line], width: u16) -> Vec<Line> {
    lines
        .iter()
        .flat_map(|line| super::soft_wrap::soft_wrap_line(line, width))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::frame::Frame;

    #[test]
    fn visual_frame_from_frame_soft_wraps_and_splits() {
        let frame = Frame::new(vec![Line::new("abcdef")]).with_cursor(Cursor {
            row: 0,
            col: 5,
            is_visible: true,
        });

        let visual = VisualFrame::from_frame(&frame, Size::from((3, 5)), 0);
        assert_eq!(
            visual.visible_lines(),
            &[Line::new("abc"), Line::new("def")]
        );
        assert_eq!(visual.cursor().row, 1);
        assert_eq!(visual.cursor().col, 2);
        assert_eq!(visual.overflow(), 0);
    }

    #[test]
    fn visual_frame_splits_overflow_from_visible_lines() {
        let frame = Frame::new(vec![
            Line::new("L1"),
            Line::new("L2"),
            Line::new("L3"),
            Line::new("L4"),
        ])
        .with_cursor(Cursor {
            row: 3,
            col: 0,
            is_visible: true,
        });

        let visual = VisualFrame::from_frame(&frame, Size::from((80, 2)), 0);
        assert_eq!(
            visual.scrollback_lines(),
            &[Line::new("L1"), Line::new("L2")]
        );
        assert_eq!(visual.visible_lines(), &[Line::new("L3"), Line::new("L4")]);
        assert_eq!(visual.cursor().row, 1);
        assert_eq!(visual.cursor().col, 0);
        assert_eq!(visual.overflow(), 2);
    }

    #[test]
    fn visual_frame_skips_already_flushed_overflow() {
        let frame = Frame::new(vec![
            Line::new("L1"),
            Line::new("L2"),
            Line::new("L3"),
            Line::new("L4"),
            Line::new("L5"),
        ])
        .with_cursor(Cursor {
            row: 4,
            col: 0,
            is_visible: true,
        });

        let visual = VisualFrame::from_frame(&frame, Size::from((80, 2)), 1);

        assert_eq!(
            visual.scrollback_lines(),
            &[Line::new("L2"), Line::new("L3")]
        );
        assert_eq!(visual.visible_lines(), &[Line::new("L4"), Line::new("L5")]);
        assert_eq!(visual.cursor().row, 1);
        assert_eq!(visual.overflow(), 3);
    }

    #[test]
    fn visual_frame_cursor_in_scrollback_gets_clamped() {
        let frame = Frame::new(vec![Line::new("L1"), Line::new("L2"), Line::new("L3")])
            .with_cursor(Cursor {
                row: 0,
                col: 0,
                is_visible: true,
            });

        let visual = VisualFrame::from_frame(&frame, Size::from((80, 2)), 0);
        assert_eq!(visual.cursor().row, 0);
        assert_eq!(visual.visible_lines().len(), 2);
    }

    #[test]
    fn visual_frame_empty_frame() {
        let frame = Frame::new(vec![]);
        let visual = VisualFrame::from_frame(&frame, Size::from((80, 24)), 0);
        assert!(visual.scrollback_lines().is_empty());
        assert!(visual.visible_lines().is_empty());
    }

    #[test]
    fn visual_frame_zero_width_keeps_lines_unwrapped() {
        let frame = Frame::new(vec![Line::new("abcdef")]).with_cursor(Cursor {
            row: 0,
            col: 3,
            is_visible: true,
        });

        let visual = VisualFrame::from_frame(&frame, Size::from((0, 5)), 0);
        assert_eq!(visual.visible_lines(), &[Line::new("abcdef")]);
        assert_eq!(visual.cursor().col, 0);
    }

    #[test]
    fn prepare_lines_for_scrollback_matches_visual_frame_wrapping() {
        let lines = vec![Line::new("abcdef")];
        let visual_frame_lines = {
            let frame = Frame::new(lines.clone()).with_cursor(Cursor {
                row: 0,
                col: 0,
                is_visible: true,
            });
            let visual = VisualFrame::from_frame(&frame, Size::from((3, 5)), 0);
            visual.visible_lines().to_vec()
        };

        let scrollback_lines = prepare_lines_for_scrollback(&lines, 3);
        assert_eq!(visual_frame_lines, scrollback_lines);
    }

    fn visual(lines: &[&str]) -> VisualFrame {
        let frame = Frame::new(lines.iter().map(|l| Line::new(*l)).collect()).with_cursor(Cursor {
            row: lines.len().saturating_sub(1),
            col: 0,
            is_visible: true,
        });
        VisualFrame::from_frame(&frame, Size::from((80, 24)), 0)
    }

    #[test]
    fn diff_identical_frames_returns_none() {
        let a = visual(&["hello", "world"]);
        let b = visual(&["hello", "world"]);
        assert!(a.diff(&b).is_none());
    }

    #[test]
    fn diff_empty_to_nonempty_returns_full_rewrite() {
        let empty = VisualFrame::empty();
        let b = visual(&["hello", "world"]);
        let diff = empty.diff(&b).unwrap();
        assert_eq!(diff.rewrite_from, 0);
        assert_eq!(diff.lines.len(), 2);
        assert_eq!(diff.previous_row_count, 0);
    }

    #[test]
    fn diff_changed_middle_line() {
        let a = visual(&["aaa", "bbb", "ccc"]);
        let b = visual(&["aaa", "BBB", "ccc"]);
        let diff = a.diff(&b).unwrap();
        assert_eq!(diff.rewrite_from, 1);
        assert_eq!(diff.lines.len(), 2);
        assert_eq!(diff.previous_row_count, 3);
    }

    #[test]
    fn diff_appended_line() {
        let a = visual(&["aaa", "bbb"]);
        let b = visual(&["aaa", "bbb", "ccc"]);
        let diff = a.diff(&b).unwrap();
        assert_eq!(diff.rewrite_from, 2);
        assert_eq!(diff.lines.len(), 1);
        assert_eq!(diff.previous_row_count, 2);
    }
}
