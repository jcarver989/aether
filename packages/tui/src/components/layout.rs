use crate::rendering::frame::Frame;

/// Stacks frames vertically, delegating cursor and line composition to
/// [`Frame::vstack`].
///
/// Each section is a [`Frame`] that already carries its own cursor. The first
/// section with a visible cursor wins, and its row is offset by the cumulative
/// line count of preceding sections.
pub struct Layout {
    sections: Vec<Frame>,
}

impl Layout {
    pub fn new() -> Self {
        Self { sections: Vec::new() }
    }

    /// Add a section to the layout.
    pub fn section(&mut self, frame: Frame) {
        self.sections.push(frame);
    }

    /// Stack the accumulated sections into a single frame.
    pub fn into_frame(self) -> Frame {
        Frame::vstack(self.sections)
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
    use crate::rendering::frame::Cursor;
    use crate::rendering::line::Line;

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
        layout.section(Frame::new(vec![Line::new("a1"), Line::new("a2")]));
        layout.section(Frame::new(vec![Line::new("b1")]));
        let frame = layout.into_frame();
        assert_eq!(frame.lines().len(), 3);
        assert_eq!(frame.lines()[0].plain_text(), "a1");
        assert_eq!(frame.lines()[2].plain_text(), "b1");
    }

    #[test]
    fn cursor_offset_is_computed_from_section_position() {
        let mut layout = Layout::new();
        layout.section(Frame::new(vec![Line::new("header1"), Line::new("header2")]));
        layout.section(Frame::new(vec![Line::new("input")]).with_cursor(Cursor::visible(0, 5)));
        layout.section(Frame::new(vec![Line::new("footer")]));

        let frame = layout.into_frame();
        assert_eq!(frame.cursor().row, 2); // 2 header lines
        assert_eq!(frame.cursor().col, 5);
        assert!(frame.cursor().is_visible);
    }

    #[test]
    fn cursor_row_adds_section_offset_and_local_row() {
        let mut layout = Layout::new();
        layout.section(Frame::new(vec![Line::new("a")]));
        layout.section(
            Frame::new(vec![Line::new("b1"), Line::new("b2"), Line::new("b3")]).with_cursor(Cursor::visible(2, 3)),
        );

        let frame = layout.into_frame();
        assert_eq!(frame.cursor().row, 3); // 1 + 2
        assert_eq!(frame.cursor().col, 3);
    }
}
