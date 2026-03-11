use super::line::Line;

/// Logical cursor position within a component's rendered output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
