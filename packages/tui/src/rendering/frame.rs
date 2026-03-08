use super::line::Line;
use super::prepared_frame::PreparedFrame;
use super::size::Size;
use super::soft_wrap::soft_wrap_lines_with_map;

/// Logical cursor position within a component's rendered output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub is_visible: bool,
}

/// Logical component output: lines plus cursor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    lines: Vec<Line>,
    cursor: Cursor,
}

impl Frame {
    pub fn new(lines: Vec<Line>, cursor: Cursor) -> Self {
        Self { lines, cursor }
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn soft_wrap(self, width: u16) -> Self {
        let (lines, logical_to_visual) = soft_wrap_lines_with_map(&self.lines, width);

        let mut cursor_row = logical_to_visual
            .get(self.cursor.row)
            .copied()
            .unwrap_or_else(|| lines.len().saturating_sub(1));

        let mut cursor_col = self.cursor.col;
        let width = usize::from(width);
        if width > 0 {
            cursor_row += cursor_col / width;
            cursor_col %= width;
        } else {
            cursor_col = 0;
        }

        Self {
            lines,
            cursor: Cursor {
                row: cursor_row,
                col: cursor_col,
                is_visible: self.cursor.is_visible,
            },
        }
    }

    pub fn clamp_cursor(mut self) -> Self {
        if self.cursor.row >= self.lines.len() {
            self.cursor.row = self.lines.len().saturating_sub(1);
        }
        self
    }

    pub fn prepare(self, size: Size, flushed_visual_count: usize) -> PreparedFrame {
        PreparedFrame::new(&self.lines, self.cursor, size, flushed_visual_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_wrap_maps_cursor_into_visual_rows() {
        let frame = Frame::new(
            vec![Line::new("abcdef")],
            Cursor {
                row: 0,
                col: 5,
                is_visible: true,
            },
        );

        let frame = frame.soft_wrap(3);

        assert_eq!(frame.lines(), &[Line::new("abc"), Line::new("def")]);
        assert_eq!(frame.cursor().row, 1);
        assert_eq!(frame.cursor().col, 2);
    }

    #[test]
    fn clamp_cursor_clamps_out_of_bounds_row() {
        let frame = Frame::new(
            vec![Line::new("a")],
            Cursor {
                row: 10,
                col: 100,
                is_visible: true,
            },
        );

        let frame = frame.clamp_cursor();

        assert_eq!(frame.cursor().row, 0);
        assert_eq!(frame.cursor().col, 100);
    }
}
