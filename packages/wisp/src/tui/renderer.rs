use super::component::RenderContext;
use super::screen::{Line, Screen};
use super::soft_wrap::soft_wrap_lines_with_map;
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
}

impl<T: Write> Renderer<T> {
    pub fn new(writer: T) -> Self {
        Self {
            writer,
            screen: Screen::new(),
            context: RenderContext::new((0, 0)),
            cursor_row_offset: 0,
            cursor_visible: true,
        }
    }

    pub fn render<C: CursorComponent + ?Sized>(&mut self, root: &mut C) -> io::Result<()> {
        let output = root.render_with_cursor(&self.context);
        let (visual_lines, logical_to_visual) =
            soft_wrap_lines_with_map(&output.lines, self.context.size.0);

        let mut cursor_row = logical_to_visual
            .get(output.cursor.logical_row)
            .copied()
            .unwrap_or_else(|| visual_lines.len().saturating_sub(1));

        let width = usize::from(self.context.size.0);
        let mut cursor_col = output.cursor.col;
        if width > 0 {
            cursor_row += cursor_col / width;
            cursor_col %= width;
        } else {
            cursor_col = 0;
        }

        if cursor_row >= visual_lines.len() {
            cursor_row = visual_lines.len().saturating_sub(1);
        }

        self.restore_cursor_position()?;
        self.screen
            .render(&visual_lines, self.context.size.0, &mut self.writer)?;

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
    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()> {
        self.restore_cursor_position()?;
        self.screen
            .push_to_scrollback(lines, self.context.size.0, &mut self.writer)
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

    pub fn update_context(&mut self, size: (u16, u16)) {
        self.context = RenderContext::new(size);
    }

    pub fn update_render_context_from_terminal(&mut self) {
        let size = match crossterm::terminal::size() {
            Ok(size) => size,
            Err(e) => {
                eprintln!("Failed to get size: {e}");
                (80, 24)
            }
        };
        self.context = RenderContext::new(size);
    }

    pub fn update_render_context(&mut self) {
        self.update_render_context_from_terminal();
    }

    pub fn update_render_context_with(&mut self, size: (u16, u16)) {
        self.update_context(size);
    }

    pub fn context(&self) -> &RenderContext {
        &self.context
    }

    #[allow(dead_code)]
    pub fn writer(&self) -> &T {
        &self.writer
    }

    #[allow(dead_code)]
    pub fn screen(&self) -> &Screen {
        &self.screen
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
}
