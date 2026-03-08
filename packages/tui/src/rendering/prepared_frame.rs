use super::frame::Cursor;
use super::line::Line;
use super::size::Size;

/// Final visual frame ready to apply to the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedFrame {
    scrollback_lines: Vec<Line>,
    visible_lines: Vec<Line>,
    cursor: Cursor,
    overflow: usize,
}

impl PreparedFrame {
    pub fn new(lines: &[Line], cursor: Cursor, size: Size, flushed_visual_count: usize) -> Self {
        let mut cursor_row = cursor.row.min(lines.len().saturating_sub(1));

        let viewport_rows = usize::from(size.height.max(1));
        let overflow = lines.len().saturating_sub(viewport_rows);
        let scrollback_lines = if overflow > flushed_visual_count {
            lines[flushed_visual_count..overflow].to_vec()
        } else {
            Vec::new()
        };

        let visible_lines = lines[overflow..].to_vec();
        cursor_row = cursor_row.saturating_sub(overflow);
        if cursor_row >= visible_lines.len() {
            cursor_row = visible_lines.len().saturating_sub(1);
        }

        Self {
            scrollback_lines,
            visible_lines,
            cursor: Cursor {
                row: cursor_row,
                col: cursor.col,
                is_visible: cursor.is_visible,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_frame_splits_overflow_from_visible_lines() {
        let prepared = PreparedFrame::new(
            &[
                Line::new("L1"),
                Line::new("L2"),
                Line::new("L3"),
                Line::new("L4"),
            ],
            Cursor {
                row: 3,
                col: 0,
                is_visible: true,
            },
            Size::from((80, 2)),
            0,
        );

        assert_eq!(
            prepared.scrollback_lines(),
            &[Line::new("L1"), Line::new("L2")]
        );
        assert_eq!(
            prepared.visible_lines(),
            &[Line::new("L3"), Line::new("L4")]
        );
        assert_eq!(prepared.cursor().row, 1);
        assert_eq!(prepared.cursor().col, 0);
        assert_eq!(prepared.overflow(), 2);
    }

    #[test]
    fn prepared_frame_skips_already_flushed_overflow() {
        let prepared = PreparedFrame::new(
            &[
                Line::new("L1"),
                Line::new("L2"),
                Line::new("L3"),
                Line::new("L4"),
                Line::new("L5"),
            ],
            Cursor {
                row: 4,
                col: 0,
                is_visible: true,
            },
            Size::from((80, 2)),
            1,
        );

        assert_eq!(
            prepared.scrollback_lines(),
            &[Line::new("L2"), Line::new("L3")]
        );
        assert_eq!(
            prepared.visible_lines(),
            &[Line::new("L4"), Line::new("L5")]
        );
        assert_eq!(prepared.cursor().row, 1);
        assert_eq!(prepared.overflow(), 3);
    }

    #[test]
    fn prepared_frame_maps_visual_cursor_into_viewport() {
        let prepared = PreparedFrame::new(
            &[Line::new("abc"), Line::new("def")],
            Cursor {
                row: 1,
                col: 2,
                is_visible: false,
            },
            Size::from((3, 5)),
            0,
        );

        assert_eq!(
            prepared.visible_lines(),
            &[Line::new("abc"), Line::new("def")]
        );
        assert_eq!(prepared.cursor().row, 1);
        assert_eq!(prepared.cursor().col, 2);
        assert!(!prepared.cursor().is_visible);
    }
}
