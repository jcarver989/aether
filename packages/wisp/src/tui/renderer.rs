use super::component::RenderContext;
use super::screen::{Line, Screen};
use super::soft_wrap::{soft_wrap_line, soft_wrap_lines_with_map};
use crossterm::QueueableCommand;
use crossterm::cursor::{Hide, MoveDown, Show};
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub logical_row: usize,
    pub col: usize,
}

pub struct RenderOutput {
    pub lines: Vec<Line>,
    pub cursor: Cursor,
    pub cursor_visible: bool,
}

pub trait CursorComponent {
    fn render_with_cursor(&mut self, context: &RenderContext) -> RenderOutput;
}

/// Pure TUI renderer that owns terminal output, frame diffing, and cursor state.
pub struct Renderer<T: Write> {
    writer: T,
    screen: Screen,
    context: RenderContext,
    /// How many rows above the last frame line the cursor currently sits.
    /// 0 = cursor at last line (Screen's default assumption).
    cursor_row_offset: u16,
    cursor_visible: bool,
    /// How many visual lines have already been flushed to scrollback
    /// via progressive overflow handling.
    flushed_visual_count: usize,
}

impl<T: Write> Renderer<T> {
    pub fn new(writer: T) -> Self {
        Self {
            writer,
            screen: Screen::new(),
            context: RenderContext::new((0, 0)),
            cursor_row_offset: 0,
            cursor_visible: true,
            flushed_visual_count: 0,
        }
    }

    pub fn render<C: CursorComponent + ?Sized>(&mut self, root: &mut C) -> io::Result<()> {
        let output = root.render_with_cursor(&self.context);
        let (full_visual_lines, logical_to_visual) =
            soft_wrap_lines_with_map(&output.lines, self.context.size.0);

        let mut cursor_row = logical_to_visual
            .get(output.cursor.logical_row)
            .copied()
            .unwrap_or_else(|| full_visual_lines.len().saturating_sub(1));

        let width = usize::from(self.context.size.0);
        let mut cursor_col = output.cursor.col;
        if width > 0 {
            cursor_row += cursor_col / width;
            cursor_col %= width;
        } else {
            cursor_col = 0;
        }

        if cursor_row >= full_visual_lines.len() {
            cursor_row = full_visual_lines.len().saturating_sub(1);
        }

        // Progressively flush overflow lines to terminal scrollback so the
        // user can scroll up to see the full response.
        let viewport_rows = usize::from(self.context.size.1.max(1));
        let overflow = full_visual_lines.len().saturating_sub(viewport_rows);

        if overflow > self.flushed_visual_count {
            let new_scrollback = &full_visual_lines[self.flushed_visual_count..overflow];
            self.restore_cursor_position()?;
            self.screen
                .push_to_scrollback(new_scrollback, &mut self.writer)?;
            self.flushed_visual_count = overflow;
        }

        // Content may have shrunk — clamp flush count
        let effective_flush = self.flushed_visual_count.min(full_visual_lines.len());
        let visual_lines = &full_visual_lines[effective_flush..];
        cursor_row = cursor_row.saturating_sub(effective_flush);
        if cursor_row >= visual_lines.len() {
            cursor_row = visual_lines.len().saturating_sub(1);
        }

        self.restore_cursor_position()?;
        self.screen
            .render(visual_lines, self.context.size.0, &mut self.writer)?;

        // Show or hide the cursor based on the component's request.
        if output.cursor_visible != self.cursor_visible {
            if output.cursor_visible {
                self.writer.queue(Show)?;
            } else {
                self.writer.queue(Hide)?;
            }
            self.cursor_visible = output.cursor_visible;
        }

        let rows_up = u16::try_from(
            visual_lines
                .len()
                .saturating_sub(1)
                .saturating_sub(cursor_row),
        )
        .unwrap_or(u16::MAX);
        self.reposition_cursor(rows_up, u16::try_from(cursor_col).unwrap_or(u16::MAX))?;
        Ok(())
    }

    /// Commit lines to permanent scrollback, replacing the managed region.
    ///
    /// Lines that were already progressively flushed during `render()` are
    /// skipped to avoid duplication in the terminal transcript.
    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()> {
        self.restore_cursor_position()?;

        let width = self.context.size.0;
        let visual: Vec<Line> = lines
            .iter()
            .flat_map(|l| soft_wrap_line(l, width))
            .collect();

        let remaining = &visual[self.flushed_visual_count.min(visual.len())..];
        self.screen
            .push_to_scrollback(remaining, &mut self.writer)?;

        self.flushed_visual_count = 0;
        Ok(())
    }

    /// Move the cursor to an absolute position relative to the end of the frame.
    ///
    /// `rows_up`: how many rows above the last frame line to place the cursor.
    /// `col`: column to move to (0-based, after a `\r`).
    pub fn reposition_cursor(&mut self, rows_up: u16, col: u16) -> io::Result<()> {
        use crossterm::cursor::{MoveRight, MoveUp};

        self.writer.queue(MoveUp(rows_up))?;
        write!(self.writer, "\r")?;
        if col > 0 {
            self.writer.queue(MoveRight(col))?;
        }
        self.writer.flush()?;
        self.cursor_row_offset = rows_up;
        Ok(())
    }

    pub fn update_render_context(&mut self) {
        let size = match crossterm::terminal::size() {
            Ok(size) => size,
            Err(e) => {
                eprintln!("Failed to get size: {e}");
                (80, 24)
            }
        };
        self.context = RenderContext::new(size);
    }

    pub fn update_render_context_with(&mut self, size: (u16, u16)) {
        self.context = RenderContext::new(size);
    }

    pub fn context(&self) -> &RenderContext {
        &self.context
    }

    #[allow(dead_code)]
    pub fn writer(&self) -> &T {
        &self.writer
    }

    #[cfg(test)]
    pub fn writer_mut(&mut self) -> &mut T {
        &mut self.writer
    }

    #[allow(dead_code)]
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    #[cfg(test)]
    pub fn flushed_visual_count(&self) -> usize {
        self.flushed_visual_count
    }

    fn restore_cursor_position(&mut self) -> io::Result<()> {
        if self.cursor_row_offset > 0 {
            self.writer.queue(MoveDown(self.cursor_row_offset))?;
            self.cursor_row_offset = 0;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeWriter {
        bytes: Vec<u8>,
    }

    impl FakeWriter {
        fn new() -> Self {
            Self { bytes: Vec::new() }
        }
    }

    impl Write for FakeWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct StubRoot {
        lines: Vec<Line>,
        cursor: Cursor,
    }

    impl CursorComponent for StubRoot {
        fn render_with_cursor(&mut self, _context: &RenderContext) -> RenderOutput {
            RenderOutput {
                lines: self.lines.clone(),
                cursor: self.cursor,
                cursor_visible: true,
            }
        }
    }

    #[test]
    fn render_soft_wraps_before_diffing() {
        let mut renderer = Renderer::new(FakeWriter::new());
        renderer.update_render_context_with((3, 20));

        let mut root = StubRoot {
            lines: vec![Line::new("abcdef")],
            cursor: Cursor {
                logical_row: 0,
                col: 5,
            },
        };

        renderer.render(&mut root).unwrap();

        assert_eq!(
            renderer.screen().prev_frame(),
            &[Line::new("abc"), Line::new("def")]
        );
    }

    #[test]
    fn push_to_scrollback_soft_wraps_long_lines() {
        let mut renderer = Renderer::new(FakeWriter::new());
        renderer.update_render_context_with((5, 20));

        // Render a short line so prev_frame is populated
        let mut root = StubRoot {
            lines: vec![Line::new("abcde")],
            cursor: Cursor {
                logical_row: 0,
                col: 0,
            },
        };
        renderer.render(&mut root).unwrap();

        // Push a 10-char line into scrollback at width 5 — should soft-wrap
        renderer
            .push_to_scrollback(&[Line::new("0123456789")])
            .unwrap();

        let output = String::from_utf8_lossy(&renderer.writer().bytes);
        // Without soft-wrapping the full "0123456789" is written as one run.
        // With soft-wrapping it becomes two visual lines: "01234" and "56789".
        assert!(
            output.contains("01234"),
            "expected wrapped first half, got: {output}"
        );
        assert!(
            output.contains("56789"),
            "expected wrapped second half, got: {output}"
        );
        // The two halves must NOT appear as one contiguous string
        assert!(
            !output.contains("0123456789"),
            "line should have been split by soft-wrap, got: {output}"
        );
    }

    #[test]
    fn out_of_bounds_cursor_clamps_without_panicking() {
        let mut renderer = Renderer::new(FakeWriter::new());
        renderer.update_render_context_with((4, 20));

        let mut root = StubRoot {
            lines: vec![Line::new("a")],
            cursor: Cursor {
                logical_row: 10,
                col: 100,
            },
        };

        renderer.render(&mut root).unwrap();
        assert_eq!(renderer.screen().prev_frame(), &[Line::new("a")]);
    }

    #[test]
    fn render_flushes_overflow_to_scrollback() {
        let mut renderer = Renderer::new(FakeWriter::new());
        // viewport height = 3 rows
        renderer.update_render_context_with((20, 3));

        let mut root = StubRoot {
            lines: vec![
                Line::new("L1"),
                Line::new("L2"),
                Line::new("L3"),
                Line::new("L4"),
                Line::new("L5"),
            ],
            cursor: Cursor {
                logical_row: 4,
                col: 0,
            },
        };

        renderer.render(&mut root).unwrap();

        // The managed frame should contain only the bottom 3 lines
        assert_eq!(
            renderer.screen().prev_frame(),
            &[Line::new("L3"), Line::new("L4"), Line::new("L5")]
        );

        // The overflow lines (L1, L2) should have been written to scrollback
        let output = String::from_utf8_lossy(&renderer.writer().bytes);
        assert!(
            output.contains("L1"),
            "L1 should be in scrollback: {output}"
        );
        assert!(
            output.contains("L2"),
            "L2 should be in scrollback: {output}"
        );
    }

    #[test]
    fn render_progressively_flushes_overflow() {
        let mut renderer = Renderer::new(FakeWriter::new());
        // viewport height = 3 rows
        renderer.update_render_context_with((20, 3));

        // First render: 4 lines → 1 overflows
        let mut root = StubRoot {
            lines: vec![
                Line::new("L1"),
                Line::new("L2"),
                Line::new("L3"),
                Line::new("L4"),
            ],
            cursor: Cursor {
                logical_row: 3,
                col: 0,
            },
        };
        renderer.render(&mut root).unwrap();
        assert_eq!(renderer.flushed_visual_count(), 1);

        let output_after_first = String::from_utf8_lossy(&renderer.writer().bytes).to_string();
        assert!(
            output_after_first.contains("L1"),
            "L1 should be flushed: {output_after_first}"
        );

        // Clear writer to track only new writes
        renderer.writer_mut().bytes.clear();

        // Second render: 6 lines → 3 overflow, but 1 already flushed → flush L2, L3
        root.lines = vec![
            Line::new("L1"),
            Line::new("L2"),
            Line::new("L3"),
            Line::new("L4"),
            Line::new("L5"),
            Line::new("L6"),
        ];
        root.cursor.logical_row = 5;
        renderer.render(&mut root).unwrap();
        assert_eq!(renderer.flushed_visual_count(), 3);

        let output_after_second = String::from_utf8_lossy(&renderer.writer().bytes).to_string();
        assert!(
            output_after_second.contains("L2"),
            "L2 should be in delta flush: {output_after_second}"
        );
        assert!(
            output_after_second.contains("L3"),
            "L3 should be in delta flush: {output_after_second}"
        );

        // Managed frame should be the bottom 3
        assert_eq!(
            renderer.screen().prev_frame(),
            &[Line::new("L4"), Line::new("L5"), Line::new("L6")]
        );
    }

    #[test]
    fn push_to_scrollback_resets_flushed_count() {
        let mut renderer = Renderer::new(FakeWriter::new());
        renderer.update_render_context_with((20, 3));

        // Render 5 lines (2 overflow) to set flushed_visual_count
        let mut root = StubRoot {
            lines: vec![
                Line::new("L1"),
                Line::new("L2"),
                Line::new("L3"),
                Line::new("L4"),
                Line::new("L5"),
            ],
            cursor: Cursor {
                logical_row: 4,
                col: 0,
            },
        };
        renderer.render(&mut root).unwrap();
        assert_eq!(renderer.flushed_visual_count(), 2);

        // Push to scrollback should reset flushed count
        renderer
            .push_to_scrollback(&[Line::new("committed")])
            .unwrap();
        assert_eq!(renderer.flushed_visual_count(), 0);
    }
}
