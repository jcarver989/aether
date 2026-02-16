use super::component::{Component, RenderContext};
use super::screen::{Line, Screen};
use crossterm::QueueableCommand;
use crossterm::cursor::MoveDown;
use std::io::{self, Write};

/// Pure TUI rendering engine.
///
/// Owns the terminal writer, diff-based `Screen`, and `RenderContext`.
/// All cursor bookkeeping (restore before render, reposition after) lives here
/// so that higher-level code only deals with component trees and scrollback lines.
pub struct FrameRenderer<T: Write> {
    writer: T,
    screen: Screen,
    context: RenderContext,
    /// How many rows above the last frame line the cursor currently sits.
    /// 0 = cursor at last line (Screen's default assumption).
    cursor_row_offset: u16,
}

impl<T: Write> FrameRenderer<T> {
    pub fn new(writer: T) -> Self {
        Self {
            writer,
            screen: Screen::new(),
            context: RenderContext::new((0, 0)),
            cursor_row_offset: 0,
        }
    }

    /// Render a component tree as the current frame.
    ///
    /// Restores the cursor to end-of-frame first, then delegates the
    /// rendered lines to `Screen::render` for diff-based output.
    pub fn render_frame(&mut self, root: &dyn Component) -> io::Result<()> {
        self.restore_cursor_position()?;
        let frame = root.render(&self.context);
        self.screen
            .render(&frame, self.context.size.0, &mut self.writer)?;
        Ok(())
    }

    /// Commit lines to permanent scrollback, replacing the managed region.
    ///
    /// Restores the cursor first, then delegates to `Screen::push_to_scrollback`.
    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()> {
        self.restore_cursor_position()?;
        self.screen.push_to_scrollback(lines, &mut self.writer)
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

    pub fn update_context_from_terminal(&mut self) {
        let sz = match crossterm::terminal::size() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to get size: {e}");
                (80, 24)
            }
        };
        self.context = RenderContext::new(sz);
    }

    pub fn context(&self) -> &RenderContext {
        &self.context
    }

    #[allow(dead_code)]
    pub fn writer(&self) -> &T {
        &self.writer
    }

    #[allow(dead_code)]
    pub fn writer_mut(&mut self) -> &mut T {
        &mut self.writer
    }

    #[allow(dead_code)]
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// Move cursor back to end-of-frame if it was repositioned.
    fn restore_cursor_position(&mut self) -> io::Result<()> {
        if self.cursor_row_offset > 0 {
            self.writer.queue(MoveDown(self.cursor_row_offset))?;
            self.cursor_row_offset = 0;
        }
        Ok(())
    }
}
