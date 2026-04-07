use super::line::Line;
use super::soft_wrap::{soft_wrap_lines_with_map, truncate_line};

/// Logical cursor position within a component's rendered output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub is_visible: bool,
}

impl Cursor {
    /// Create a hidden cursor at position (0, 0).
    pub fn hidden() -> Self {
        Self::default()
    }

    /// Create a visible cursor at the given position.
    pub fn visible(row: usize, col: usize) -> Self {
        Self { row, col, is_visible: true }
    }
}

/// Overflow policy used by [`Frame::fit`] when content exceeds the target width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    /// Wrap rows that exceed the width onto additional visual rows.
    Wrap,
    /// Truncate rows that exceed the width. Row count is preserved.
    Truncate,
}

/// Configuration for [`Frame::fit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FitOptions {
    pub overflow_x: Overflow,
    /// When true, every resulting row is padded to `width` so the row visually
    /// fills its allocated box. Padding inherits any background color present
    /// on the row.
    pub fill_x: bool,
}

impl FitOptions {
    /// Wrapping fit, no fill.
    pub fn wrap() -> Self {
        Self { overflow_x: Overflow::Wrap, fill_x: false }
    }

    /// Truncating fit, no fill.
    pub fn truncate() -> Self {
        Self { overflow_x: Overflow::Truncate, fill_x: false }
    }

    /// Builder: enable row fill.
    pub fn with_fill(mut self) -> Self {
        self.fill_x = true;
        self
    }
}

/// A horizontally-stacked slot in [`Frame::hstack`].
///
/// Each part holds a child frame and the width it occupies in the composed
/// output. The child frame is assumed to already fit `width`; callers should
/// `fit(width, ...)` first if it might not.
#[derive(Debug, Clone)]
pub struct FramePart {
    pub frame: Frame,
    pub width: u16,
}

impl FramePart {
    pub fn new(frame: Frame, width: u16) -> Self {
        Self { frame, width }
    }
}

#[doc = include_str!("../docs/frame.md")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    lines: Vec<Line>,
    cursor: Cursor,
}

impl Frame {
    pub fn new(lines: Vec<Line>) -> Self {
        Self { lines, cursor: Cursor::hidden() }
    }

    /// An empty frame with a hidden cursor.
    pub fn empty() -> Self {
        Self { lines: Vec::new(), cursor: Cursor::hidden() }
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Replace the cursor without cloning lines.
    pub fn with_cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = cursor;
        self
    }

    pub fn into_lines(self) -> Vec<Line> {
        self.lines
    }

    pub fn into_parts(self) -> (Vec<Line>, Cursor) {
        (self.lines, self.cursor)
    }

    pub fn clamp_cursor(mut self) -> Self {
        if self.cursor.row >= self.lines.len() {
            self.cursor.row = self.lines.len().saturating_sub(1);
        }
        self
    }

    /// Fit the frame to a target width.
    ///
    /// - [`Overflow::Wrap`]: each line is soft-wrapped to `width`. The cursor
    ///   row is remapped to the wrapped visual row, and the cursor column is
    ///   reduced modulo `width`, with the row advanced for any overflow. This
    ///   matches the wrap math used by [`VisualFrame`](super::visual_frame::VisualFrame).
    /// - [`Overflow::Truncate`]: each line is truncated to `width`. Row count
    ///   is unchanged. The cursor column is clamped to `width.saturating_sub(1)`.
    /// - `fill_x`: every resulting row is marked with row-fill metadata using
    ///   any background color present on the row. The fill is **not**
    ///   materialized into trailing spaces here — that happens later, in
    ///   [`Frame::hstack`] (per slot width) or
    ///   [`VisualFrame`](super::visual_frame::VisualFrame) (per terminal width).
    ///   Deferring materialization is what prevents trailing-space rows from
    ///   producing phantom rows when wrapped again at a smaller width.
    ///
    /// As a special case, `width == 0` returns the frame unchanged with a
    /// hidden cursor (matching the zero-width behavior of `VisualFrame`).
    pub fn fit(self, width: u16, options: FitOptions) -> Self {
        if width == 0 {
            return Self { lines: self.lines, cursor: Cursor::hidden() };
        }

        match options.overflow_x {
            Overflow::Wrap => self.fit_wrap(width, options.fill_x),
            Overflow::Truncate => self.fit_truncate(width, options.fill_x),
        }
    }

    /// Shift visual content `cols` columns to the right by prepending spaces
    /// to each line. The cursor column is shifted by `cols`. The row is
    /// unchanged.
    ///
    /// The prepended prefix inherits any background color from the line, so
    /// row-fill highlights extend through the indent.
    pub fn indent(self, cols: u16) -> Self {
        if cols == 0 {
            return self;
        }
        let prefix = " ".repeat(usize::from(cols));
        let lines = self.lines.into_iter().map(|line| line.prepend(prefix.clone())).collect();
        let cursor = if self.cursor.is_visible {
            Cursor { row: self.cursor.row, col: self.cursor.col + usize::from(cols), is_visible: true }
        } else {
            self.cursor
        };
        Self { lines, cursor }
    }

    /// Concatenate frames vertically.
    ///
    /// The first frame in the iterator that has a visible cursor wins. Its
    /// row is offset by the cumulative line count of all preceding frames.
    pub fn vstack(frames: impl IntoIterator<Item = Frame>) -> Self {
        let mut all_lines: Vec<Line> = Vec::new();
        let mut cursor = Cursor::hidden();
        for frame in frames {
            let row_offset = all_lines.len();
            if !cursor.is_visible && frame.cursor.is_visible {
                cursor = Cursor { row: frame.cursor.row + row_offset, col: frame.cursor.col, is_visible: true };
            }
            all_lines.extend(frame.lines);
        }
        Self { lines: all_lines, cursor }
    }

    /// Compose frames horizontally into fixed-width slots.
    ///
    /// Each part's frame is assumed to already fit its `width` (callers should
    /// `fit(width, FitOptions::wrap())` or similar first). Any row-fill
    /// metadata on a part's rows is materialized to the part's slot width
    /// before merging, so trailing fill never bleeds into a neighboring slot.
    /// Heights are balanced by padding shorter frames with blank rows of the
    /// slot's width. The first part with a visible cursor wins; its column is
    /// offset by the cumulative width of preceding slots.
    pub fn hstack(parts: impl IntoIterator<Item = FramePart>) -> Self {
        let parts: Vec<FramePart> = parts.into_iter().collect();
        if parts.is_empty() {
            return Self::empty();
        }

        let max_rows = parts.iter().map(|p| p.frame.lines.len()).max().unwrap_or(0);

        let mut cursor = Cursor::hidden();
        let mut col_offset: usize = 0;
        for part in &parts {
            if !cursor.is_visible && part.frame.cursor.is_visible {
                cursor =
                    Cursor { row: part.frame.cursor.row, col: part.frame.cursor.col + col_offset, is_visible: true };
            }
            col_offset += usize::from(part.width);
        }

        let mut merged: Vec<Line> = Vec::with_capacity(max_rows);
        for row_idx in 0..max_rows {
            let mut row = Line::default();
            for part in &parts {
                let slot_width = usize::from(part.width);
                let Some(line) = part.frame.lines.get(row_idx) else {
                    row.push_text(" ".repeat(slot_width));
                    continue;
                };
                if line.fill().is_some() {
                    // Fill metadata means the line wants its trailing space
                    // styled — let extend_bg_to_width consume it so the slot
                    // background renders correctly.
                    let mut materialized = line.clone();
                    materialized.extend_bg_to_width(slot_width);
                    row.append_line(&materialized);
                } else {
                    row.append_line(line);
                    let line_width = line.display_width();
                    if line_width < slot_width {
                        row.push_text(" ".repeat(slot_width - line_width));
                    }
                }
            }
            merged.push(row);
        }

        Self { lines: merged, cursor }
    }

    fn fit_wrap(self, width: u16, fill_x: bool) -> Self {
        let (mut wrapped_lines, logical_to_visual) = soft_wrap_lines_with_map(&self.lines, width);

        let cursor = if self.cursor.is_visible {
            let mut visual_row = logical_to_visual
                .get(self.cursor.row)
                .copied()
                .unwrap_or_else(|| wrapped_lines.len().saturating_sub(1));
            let mut visual_col = self.cursor.col;
            let width_usize = usize::from(width);
            visual_row += visual_col / width_usize;
            visual_col %= width_usize;
            if visual_row >= wrapped_lines.len() {
                visual_row = wrapped_lines.len().saturating_sub(1);
            }
            Cursor { row: visual_row, col: visual_col, is_visible: true }
        } else {
            Cursor::hidden()
        };

        apply_fill_metadata(&mut wrapped_lines, fill_x);
        Self { lines: wrapped_lines, cursor }
    }

    fn fit_truncate(self, width: u16, fill_x: bool) -> Self {
        let width_usize = usize::from(width);
        let mut lines: Vec<Line> = self.lines.iter().map(|line| truncate_line(line, width_usize)).collect();

        apply_fill_metadata(&mut lines, fill_x);

        let cursor = if self.cursor.is_visible {
            let max_col = width_usize.saturating_sub(1);
            Cursor { row: self.cursor.row, col: self.cursor.col.min(max_col), is_visible: true }
        } else {
            Cursor::hidden()
        };

        Self { lines, cursor }
    }
}

fn apply_fill_metadata(lines: &mut [Line], fill_x: bool) {
    if !fill_x {
        return;
    }
    for line in lines {
        line.set_fill(line.infer_fill_style());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_hidden_returns_invisible_cursor_at_origin() {
        let cursor = Cursor::hidden();
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
        assert!(!cursor.is_visible);
    }

    #[test]
    fn cursor_visible_returns_visible_cursor_at_position() {
        let cursor = Cursor::visible(5, 10);
        assert_eq!(cursor.row, 5);
        assert_eq!(cursor.col, 10);
        assert!(cursor.is_visible);
    }

    #[test]
    fn clamp_cursor_clamps_out_of_bounds_row() {
        let frame = Frame::new(vec![Line::new("a")]).with_cursor(Cursor::visible(10, 100));
        let frame = frame.clamp_cursor();
        assert_eq!(frame.cursor().row, 0);
        assert_eq!(frame.cursor().col, 100);
    }

    #[test]
    fn with_cursor_replaces_cursor_without_cloning_lines() {
        let frame = Frame::new(vec![Line::new("hello")]);
        let new_cursor = Cursor::visible(0, 3);
        let frame = frame.with_cursor(new_cursor);
        assert_eq!(frame.cursor(), new_cursor);
        assert_eq!(frame.lines()[0].plain_text(), "hello");
    }

    // ===== Frame::fit (Wrap) =====

    #[test]
    fn fit_wrap_breaks_long_line_into_multiple_rows() {
        let frame = Frame::new(vec![Line::new("abcdef")]);
        let frame = frame.fit(3, FitOptions::wrap());
        assert_eq!(frame.lines().len(), 2);
        assert_eq!(frame.lines()[0].plain_text(), "abc");
        assert_eq!(frame.lines()[1].plain_text(), "def");
    }

    #[test]
    fn fit_wrap_remaps_cursor_on_wrapped_row() {
        let frame = Frame::new(vec![Line::new("abcdef")]).with_cursor(Cursor::visible(0, 5));
        let frame = frame.fit(3, FitOptions::wrap());
        // col 5 → row += 5/3 = 1, col = 5%3 = 2
        assert_eq!(frame.cursor().row, 1);
        assert_eq!(frame.cursor().col, 2);
        assert!(frame.cursor().is_visible);
    }

    #[test]
    fn fit_wrap_remaps_cursor_across_logical_rows() {
        let frame = Frame::new(vec![Line::new("abcdef"), Line::new("xy")]).with_cursor(Cursor::visible(1, 1));
        let frame = frame.fit(3, FitOptions::wrap());
        // logical row 0 wraps to 2 visual rows ("abc","def"), logical row 1 starts at visual row 2.
        // Cursor at logical (1, 1) → visual (2, 1).
        assert_eq!(frame.cursor().row, 2);
        assert_eq!(frame.cursor().col, 1);
    }

    #[test]
    fn fit_wrap_hides_cursor_when_logical_row_is_invisible() {
        let frame = Frame::new(vec![Line::new("abcdef")]);
        let frame = frame.fit(3, FitOptions::wrap());
        assert!(!frame.cursor().is_visible);
    }

    #[test]
    fn fit_wrap_with_fill_marks_each_row_with_fill_metadata_only() {
        // fit(...with_fill()) defers materialization. Each wrapped row has no
        // trailing spaces yet — the fill metadata is set so that hstack /
        // VisualFrame can materialize against the appropriate target width.
        let frame = Frame::new(vec![Line::new("abcdef")]);
        let frame = frame.fit(4, FitOptions::wrap().with_fill());
        assert_eq!(frame.lines().len(), 2);
        assert_eq!(frame.lines()[0].plain_text(), "abcd");
        assert_eq!(frame.lines()[1].plain_text(), "ef");
        for line in frame.lines() {
            assert!(line.fill().is_some(), "fill metadata should be set");
        }
    }

    #[test]
    fn fit_wrap_zero_width_returns_lines_unchanged_and_hides_cursor() {
        let frame = Frame::new(vec![Line::new("abc")]).with_cursor(Cursor::visible(0, 1));
        let frame = frame.fit(0, FitOptions::wrap());
        assert_eq!(frame.lines().len(), 1);
        assert_eq!(frame.lines()[0].plain_text(), "abc");
        assert!(!frame.cursor().is_visible);
    }

    // ===== Frame::fit (Truncate) =====

    #[test]
    fn fit_truncate_cuts_each_row_to_width() {
        let frame = Frame::new(vec![Line::new("abcdef"), Line::new("xyz")]);
        let frame = frame.fit(3, FitOptions::truncate());
        assert_eq!(frame.lines().len(), 2);
        assert_eq!(frame.lines()[0].plain_text(), "abc");
        assert_eq!(frame.lines()[1].plain_text(), "xyz");
    }

    #[test]
    fn fit_truncate_clamps_cursor_col_within_width() {
        let frame = Frame::new(vec![Line::new("abcdef")]).with_cursor(Cursor::visible(0, 10));
        let frame = frame.fit(3, FitOptions::truncate());
        assert_eq!(frame.cursor().row, 0);
        assert_eq!(frame.cursor().col, 2); // width - 1
        assert!(frame.cursor().is_visible);
    }

    #[test]
    fn fit_truncate_preserves_in_range_cursor() {
        let frame = Frame::new(vec![Line::new("abcdef")]).with_cursor(Cursor::visible(0, 1));
        let frame = frame.fit(5, FitOptions::truncate());
        assert_eq!(frame.cursor().col, 1);
    }

    #[test]
    fn fit_truncate_preserves_row_count() {
        let frame = Frame::new(vec![Line::new("a"), Line::new("b"), Line::new("c")]);
        let frame = frame.fit(2, FitOptions::truncate());
        assert_eq!(frame.lines().len(), 3);
    }

    #[test]
    fn fit_truncate_with_fill_marks_rows_with_fill_metadata_only() {
        let frame = Frame::new(vec![Line::new("ab")]);
        let frame = frame.fit(5, FitOptions::truncate().with_fill());
        // No materialization here — content is unchanged but fill is set.
        assert_eq!(frame.lines()[0].plain_text(), "ab");
        assert!(frame.lines()[0].fill().is_some());
    }

    // ===== Frame::indent =====

    #[test]
    fn indent_prepends_spaces_to_each_line() {
        let frame = Frame::new(vec![Line::new("a"), Line::new("b")]);
        let frame = frame.indent(2);
        assert_eq!(frame.lines()[0].plain_text(), "  a");
        assert_eq!(frame.lines()[1].plain_text(), "  b");
    }

    #[test]
    fn indent_shifts_cursor_col() {
        let frame = Frame::new(vec![Line::new("hi")]).with_cursor(Cursor::visible(0, 1));
        let frame = frame.indent(3);
        assert_eq!(frame.cursor().row, 0);
        assert_eq!(frame.cursor().col, 4);
        assert!(frame.cursor().is_visible);
    }

    #[test]
    fn indent_zero_is_noop() {
        let frame = Frame::new(vec![Line::new("hi")]).with_cursor(Cursor::visible(0, 1));
        let original_text = frame.lines()[0].plain_text();
        let original_cursor = frame.cursor();
        let frame = frame.indent(0);
        assert_eq!(frame.lines()[0].plain_text(), original_text);
        assert_eq!(frame.cursor(), original_cursor);
    }

    #[test]
    fn indent_carries_background_through_prefix() {
        use crate::style::Style;
        use crossterm::style::Color;
        let frame = Frame::new(vec![Line::with_style("hi", Style::default().bg_color(Color::Blue))]);
        let frame = frame.indent(2);
        let line = &frame.lines()[0];
        // Prefix span should carry the background color from the original line.
        assert_eq!(line.spans()[0].style().bg, Some(Color::Blue));
        assert_eq!(line.plain_text(), "  hi");
    }

    #[test]
    fn indent_does_not_make_hidden_cursor_visible() {
        let frame = Frame::new(vec![Line::new("hi")]);
        let frame = frame.indent(2);
        assert!(!frame.cursor().is_visible);
    }

    // ===== Frame::vstack =====

    #[test]
    fn vstack_empty_input_produces_empty_frame() {
        let frame = Frame::vstack(std::iter::empty());
        assert!(frame.lines().is_empty());
        assert!(!frame.cursor().is_visible);
    }

    #[test]
    fn vstack_concatenates_in_order() {
        let a = Frame::new(vec![Line::new("a1"), Line::new("a2")]);
        let b = Frame::new(vec![Line::new("b1")]);
        let frame = Frame::vstack([a, b]);
        assert_eq!(frame.lines().len(), 3);
        assert_eq!(frame.lines()[0].plain_text(), "a1");
        assert_eq!(frame.lines()[1].plain_text(), "a2");
        assert_eq!(frame.lines()[2].plain_text(), "b1");
    }

    #[test]
    fn vstack_offsets_cursor_by_preceding_line_count() {
        let a = Frame::new(vec![Line::new("a1"), Line::new("a2")]);
        let b = Frame::new(vec![Line::new("b1")]).with_cursor(Cursor::visible(0, 0));
        let frame = Frame::vstack([a, b]);
        assert_eq!(frame.cursor().row, 2);
        assert_eq!(frame.cursor().col, 0);
        assert!(frame.cursor().is_visible);
    }

    #[test]
    fn vstack_first_visible_cursor_wins() {
        let a = Frame::new(vec![Line::new("a")]).with_cursor(Cursor::visible(0, 1));
        let b = Frame::new(vec![Line::new("b")]).with_cursor(Cursor::visible(0, 5));
        let frame = Frame::vstack([a, b]);
        assert_eq!(frame.cursor().row, 0);
        assert_eq!(frame.cursor().col, 1);
    }

    #[test]
    fn vstack_no_visible_cursor_returns_hidden_cursor() {
        let a = Frame::new(vec![Line::new("a")]);
        let b = Frame::new(vec![Line::new("b")]);
        let frame = Frame::vstack([a, b]);
        assert!(!frame.cursor().is_visible);
    }

    // ===== Frame::hstack =====

    #[test]
    fn hstack_empty_input_produces_empty_frame() {
        let frame = Frame::hstack(std::iter::empty());
        assert!(frame.lines().is_empty());
        assert!(!frame.cursor().is_visible);
    }

    #[test]
    fn hstack_merges_equal_height_parts_row_by_row() {
        let left = Frame::new(vec![Line::new("aa"), Line::new("bb")]);
        let right = Frame::new(vec![Line::new("XX"), Line::new("YY")]);
        let frame = Frame::hstack([FramePart::new(left, 2), FramePart::new(right, 2)]);
        assert_eq!(frame.lines().len(), 2);
        assert_eq!(frame.lines()[0].plain_text(), "aaXX");
        assert_eq!(frame.lines()[1].plain_text(), "bbYY");
    }

    #[test]
    fn hstack_pads_shorter_part_with_blank_rows() {
        let left = Frame::new(vec![Line::new("aa"), Line::new("bb")]);
        let right = Frame::new(vec![Line::new("XX")]);
        let frame = Frame::hstack([FramePart::new(left, 2), FramePart::new(right, 2)]);
        assert_eq!(frame.lines().len(), 2);
        assert_eq!(frame.lines()[0].plain_text(), "aaXX");
        assert_eq!(frame.lines()[1].plain_text(), "bb  ");
    }

    #[test]
    fn hstack_left_visible_cursor_unchanged_col() {
        let left = Frame::new(vec![Line::new("aa")]).with_cursor(Cursor::visible(0, 1));
        let right = Frame::new(vec![Line::new("XX")]);
        let frame = Frame::hstack([FramePart::new(left, 2), FramePart::new(right, 2)]);
        assert_eq!(frame.cursor().row, 0);
        assert_eq!(frame.cursor().col, 1);
        assert!(frame.cursor().is_visible);
    }

    #[test]
    fn hstack_right_visible_cursor_offset_by_left_width() {
        let left = Frame::new(vec![Line::new("aaa")]);
        let right = Frame::new(vec![Line::new("XX")]).with_cursor(Cursor::visible(0, 1));
        let frame = Frame::hstack([FramePart::new(left, 3), FramePart::new(right, 2)]);
        assert_eq!(frame.cursor().row, 0);
        assert_eq!(frame.cursor().col, 1 + 3);
        assert!(frame.cursor().is_visible);
    }

    #[test]
    fn hstack_first_visible_cursor_wins_when_both_present() {
        let left = Frame::new(vec![Line::new("aa")]).with_cursor(Cursor::visible(0, 0));
        let right = Frame::new(vec![Line::new("XX")]).with_cursor(Cursor::visible(0, 1));
        let frame = Frame::hstack([FramePart::new(left, 2), FramePart::new(right, 2)]);
        assert_eq!(frame.cursor().col, 0);
    }

    #[test]
    fn hstack_no_visible_cursor_returns_hidden_cursor() {
        let left = Frame::new(vec![Line::new("aa")]);
        let right = Frame::new(vec![Line::new("XX")]);
        let frame = Frame::hstack([FramePart::new(left, 2), FramePart::new(right, 2)]);
        assert!(!frame.cursor().is_visible);
    }

    #[test]
    fn hstack_materializes_fill_to_each_part_slot_width() {
        use crate::style::Style;
        use crossterm::style::Color;

        let left = Frame::new(vec![
            Line::with_style("hi", Style::default().bg_color(Color::Red))
                .with_fill(Style::default().bg_color(Color::Red)),
        ]);
        let right = Frame::new(vec![Line::new("XX")]);
        let frame = Frame::hstack([FramePart::new(left, 5), FramePart::new(right, 2)]);
        // Left slot should be expanded to width 5 with red fill, then "XX" appended.
        assert_eq!(frame.lines()[0].plain_text(), "hi   XX");
        // Materialized; no fill metadata leaks through.
        assert_eq!(frame.lines()[0].fill(), None);
    }

    #[test]
    fn fit_wrap_with_fill_propagates_metadata_to_wrapped_rows() {
        use crate::style::Style;
        use crossterm::style::Color;

        let line = Line::with_style("abcdefgh", Style::default().bg_color(Color::Blue));
        let frame = Frame::new(vec![line]).fit(3, FitOptions::wrap().with_fill());
        assert_eq!(frame.lines().len(), 3);
        for row in frame.lines() {
            assert!(row.fill().is_some(), "every wrapped row should carry fill metadata");
            assert_eq!(row.fill().unwrap().bg, Some(Color::Blue));
        }
    }

    #[test]
    fn hstack_three_parts_offsets_cursor_by_cumulative_widths() {
        let left = Frame::new(vec![Line::new("aa")]);
        let mid = Frame::new(vec![Line::new("|")]);
        let right = Frame::new(vec![Line::new("XX")]).with_cursor(Cursor::visible(0, 1));
        let frame = Frame::hstack([FramePart::new(left, 2), FramePart::new(mid, 1), FramePart::new(right, 2)]);
        assert_eq!(frame.lines()[0].plain_text(), "aa|XX");
        assert_eq!(frame.cursor().col, 1 + 2 + 1);
    }
}
