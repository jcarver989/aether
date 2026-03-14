use super::line::Line;

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
        Self {
            row,
            col,
            is_visible: true,
        }
    }
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

    /// Replace the cursor without cloning lines.
    pub fn with_cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = cursor;
        self
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
        let frame = Frame::new(vec![Line::new("a")], Cursor::visible(10, 100));

        let frame = frame.clamp_cursor();

        assert_eq!(frame.cursor().row, 0);
        assert_eq!(frame.cursor().col, 100);
    }

    #[test]
    fn with_cursor_replaces_cursor_without_cloning_lines() {
        let frame = Frame::new(vec![Line::new("hello")], Cursor::hidden());
        let new_cursor = Cursor::visible(0, 3);
        let frame = frame.with_cursor(new_cursor);

        assert_eq!(frame.cursor(), new_cursor);
        assert_eq!(frame.lines()[0].plain_text(), "hello");
    }
}
