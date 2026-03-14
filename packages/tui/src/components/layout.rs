use crate::rendering::frame::{Cursor, Frame};
use crate::rendering::line::Line;

/// Stacks content sections vertically with automatic cursor offset tracking.
///
/// Replaces the manual `Panel::render_with_offsets()` + index tracking pattern.
/// Use `section()` for non-interactive content and `section_with_cursor()` for the
/// section that owns the cursor.
pub struct Layout {
    sections: Vec<Vec<Line>>,
    cursor: Option<Cursor>,
    cursor_section_index: Option<usize>,
}

impl Layout {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
            cursor: None,
            cursor_section_index: None,
        }
    }

    /// Add a content section (no cursor).
    pub fn section(&mut self, lines: Vec<Line>) {
        self.sections.push(lines);
    }

    /// Add a content section that owns the cursor.
    pub fn section_with_cursor(&mut self, lines: Vec<Line>, cursor: Cursor) {
        self.cursor_section_index = Some(self.sections.len());
        self.cursor = Some(cursor);
        self.sections.push(lines);
    }

    /// Flatten all sections into a Frame, auto-computing cursor Y offset.
    pub fn into_frame(self) -> Frame {
        let mut all_lines = Vec::new();
        let mut section_offsets = Vec::with_capacity(self.sections.len());

        for section in &self.sections {
            section_offsets.push(all_lines.len());
            all_lines.extend(section.iter().cloned());
        }

        let cursor = match (self.cursor_section_index, self.cursor) {
            (Some(idx), Some(c)) => Cursor {
                row: section_offsets[idx] + c.row,
                col: c.col,
                is_visible: c.is_visible,
            },
            _ => Cursor {
                row: 0,
                col: 0,
                is_visible: false,
            },
        };

        Frame::new(all_lines).with_cursor(cursor)
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_layout_produces_empty_frame() {
        let layout = Layout::new();
        let frame = layout.into_frame();
        assert!(frame.lines().is_empty());
        assert!(!frame.cursor().is_visible);
    }

    #[test]
    fn sections_are_stacked_in_order() {
        let mut layout = Layout::new();
        layout.section(vec![Line::new("a1"), Line::new("a2")]);
        layout.section(vec![Line::new("b1")]);
        let frame = layout.into_frame();
        assert_eq!(frame.lines().len(), 3);
        assert_eq!(frame.lines()[0].plain_text(), "a1");
        assert_eq!(frame.lines()[2].plain_text(), "b1");
    }

    #[test]
    fn cursor_offset_is_computed_from_section_position() {
        let mut layout = Layout::new();
        layout.section(vec![Line::new("header1"), Line::new("header2")]);
        layout.section_with_cursor(
            vec![Line::new("input")],
            Cursor {
                row: 0,
                col: 5,
                is_visible: true,
            },
        );
        layout.section(vec![Line::new("footer")]);

        let frame = layout.into_frame();
        assert_eq!(frame.cursor().row, 2); // 2 header lines
        assert_eq!(frame.cursor().col, 5);
        assert!(frame.cursor().is_visible);
    }

    #[test]
    fn cursor_row_adds_section_offset_and_local_row() {
        let mut layout = Layout::new();
        layout.section(vec![Line::new("a")]);
        layout.section_with_cursor(
            vec![Line::new("b1"), Line::new("b2"), Line::new("b3")],
            Cursor {
                row: 2,
                col: 3,
                is_visible: true,
            },
        );

        let frame = layout.into_frame();
        assert_eq!(frame.cursor().row, 3); // 1 + 2
        assert_eq!(frame.cursor().col, 3);
    }
}
