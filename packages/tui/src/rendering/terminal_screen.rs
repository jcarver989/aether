use crossterm::{
    QueueableCommand,
    cursor::MoveUp,
    terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate},
};
use std::io::{self, Write};

use super::line::Line;
use super::prepared_frame::PreparedFrame;
use super::soft_wrap::soft_wrap_line;

/// Terminal output state for a managed relative-cursor region.
///
/// Uses relative cursor movement (`MoveUp` + `\r`) to navigate back to the
/// start of the managed region. This avoids absolute row tracking, which breaks
/// when the terminal scrolls content upward.
///
/// **Cursor invariant:** After every render or `push_to_scrollback`, the
/// cursor sits at the end of the last managed line unless explicitly
/// repositioned afterward.
pub struct TerminalScreen<W: Write> {
    writer: W,
    prev_frame: Vec<Line>,
    last_width: u16,
    /// How many rows above the last frame line the cursor currently sits.
    /// 0 = cursor at last line.
    cursor_row_offset: u16,
    cursor_visible: bool,
    /// How many visual lines have already been flushed to scrollback
    /// via progressive overflow handling.
    flushed_visual_count: usize,
}

impl<W: Write> TerminalScreen<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            prev_frame: Vec::new(),
            last_width: 0,
            cursor_row_offset: 0,
            cursor_visible: true,
            flushed_visual_count: 0,
        }
    }

    pub fn clear_screen(&mut self) -> io::Result<()> {
        self.writer.queue(Clear(ClearType::Purge))?;
        write!(self.writer, "\x1b[H")?;
        self.writer.flush()?;
        self.prev_frame.clear();
        self.cursor_row_offset = 0;
        self.flushed_visual_count = 0;
        Ok(())
    }

    pub fn flushed_visual_count(&self) -> usize {
        self.flushed_visual_count
    }

    pub fn writer(&self) -> &W {
        &self.writer
    }

    pub fn render_frame(&mut self, frame: &PreparedFrame, width: u16) -> io::Result<()> {
        self.restore_cursor_position()?;

        if !frame.scrollback_lines().is_empty() {
            self.push_visual_to_scrollback(frame.scrollback_lines())?;
            self.flushed_visual_count = frame.overflow();
        }

        self.render_visible(frame.visible_lines(), width)?;
        self.set_cursor_visible(frame.cursor().is_visible)?;

        let rows_up = u16::try_from(
            frame
                .visible_lines()
                .len()
                .saturating_sub(1)
                .saturating_sub(frame.cursor().row),
        )
        .unwrap_or(u16::MAX);
        self.reposition_cursor(
            rows_up,
            u16::try_from(frame.cursor().col).unwrap_or(u16::MAX),
        )?;
        Ok(())
    }

    /// Commit lines to permanent scrollback, replacing the managed region.
    ///
    /// Lines that were already progressively flushed are skipped to avoid
    /// duplication in the terminal transcript.
    pub fn push_to_scrollback(&mut self, lines: &[Line], width: u16) -> io::Result<()> {
        self.restore_cursor_position()?;

        let visual: Vec<Line> = lines
            .iter()
            .flat_map(|line| soft_wrap_line(line, width))
            .collect();
        let remaining = &visual[self.flushed_visual_count.min(visual.len())..];
        self.push_visual_to_scrollback(remaining)?;
        self.flushed_visual_count = 0;
        Ok(())
    }

    fn render_visible(&mut self, new_frame: &[Line], width: u16) -> io::Result<usize> {
        let prev_on_screen = self.prev_frame.len();

        if width != self.last_width {
            self.prev_frame.clear();
            self.last_width = width;
        }

        if new_frame == self.prev_frame {
            return Ok(0);
        }

        self.writer.queue(BeginSynchronizedUpdate)?;

        let first_diff = self
            .prev_frame
            .iter()
            .zip(new_frame.iter())
            .position(|(old, new)| old != new)
            .unwrap_or(self.prev_frame.len().min(new_frame.len()));

        let rewrite_from = if new_frame.is_empty() {
            0
        } else {
            first_diff.min(new_frame.len() - 1)
        };

        if rewrite_from < prev_on_screen {
            let lines_up = prev_on_screen - 1 - rewrite_from;
            if lines_up > 0 {
                self.writer
                    .queue(MoveUp(u16::try_from(lines_up).unwrap_or(u16::MAX)))?;
            }
            write!(self.writer, "\r")?;
            self.writer.queue(Clear(ClearType::FromCursorDown))?;
        } else if prev_on_screen > 0 {
            write!(self.writer, "\r\n")?;
            self.writer.queue(Clear(ClearType::FromCursorDown))?;
        }

        let to_write = &new_frame[rewrite_from..];
        for (i, line) in to_write.iter().enumerate() {
            write!(self.writer, "{}", line.to_ansi_string())?;
            if i < to_write.len() - 1 {
                write!(self.writer, "\r\n")?;
            }
        }

        self.writer.queue(EndSynchronizedUpdate)?;
        self.writer.flush()?;
        let lines_written = to_write.len();
        self.prev_frame = new_frame.to_vec();
        Ok(lines_written)
    }

    fn push_visual_to_scrollback(&mut self, visual_lines: &[Line]) -> io::Result<()> {
        if visual_lines.is_empty() {
            return Ok(());
        }

        self.writer.queue(BeginSynchronizedUpdate)?;

        if self.prev_frame.len() > 1 {
            self.writer.queue(MoveUp(
                u16::try_from(self.prev_frame.len() - 1).unwrap_or(u16::MAX),
            ))?;
        }
        write!(self.writer, "\r")?;
        self.writer.queue(Clear(ClearType::FromCursorDown))?;

        for line in visual_lines {
            write!(self.writer, "{}\r\n", line.to_ansi_string())?;
        }

        self.writer.queue(EndSynchronizedUpdate)?;
        self.writer.flush()?;

        self.prev_frame.clear();
        Ok(())
    }

    fn set_cursor_visible(&mut self, visible: bool) -> io::Result<()> {
        use crossterm::cursor::{Hide, Show};

        if visible != self.cursor_visible {
            if visible {
                self.writer.queue(Show)?;
            } else {
                self.writer.queue(Hide)?;
            }
            self.cursor_visible = visible;
        }
        Ok(())
    }

    fn reposition_cursor(&mut self, rows_up: u16, col: u16) -> io::Result<()> {
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

    fn restore_cursor_position(&mut self) -> io::Result<()> {
        use crossterm::cursor::MoveDown;

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
    use crate::rendering::frame::{Cursor, Frame};
    use crate::rendering::size::Size;

    /// Minimal writer that captures all bytes for inspection.
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

    fn frame(lines: &[&str], width: u16, height: u16) -> PreparedFrame {
        Frame::new(
            lines.iter().map(|line| Line::new(*line)).collect(),
            Cursor {
                row: lines.len().saturating_sub(1),
                col: 0,
                is_visible: true,
            },
        )
        .soft_wrap(width)
        .clamp_cursor()
        .prepare(Size::from((width, height)), 0)
    }

    #[test]
    fn empty_to_empty_is_noop() {
        let mut terminal = TerminalScreen::new(FakeWriter::new());
        terminal.render_visible(&[], 80).unwrap();
        assert!(terminal.writer.bytes.is_empty());
    }

    #[test]
    fn first_render_writes_all_lines() {
        let mut terminal = TerminalScreen::new(FakeWriter::new());
        let frame = frame(&["hello", "world"], 80, 24);
        terminal.render_frame(&frame, 80).unwrap();
        let output = String::from_utf8_lossy(&terminal.writer.bytes);
        assert!(output.contains("hello"));
        assert!(output.contains("world"));
    }

    #[test]
    fn identical_frames_produce_no_visible_rewrites() {
        let mut terminal = TerminalScreen::new(FakeWriter::new());
        let frame = frame(&["hello", "world"], 80, 24);
        terminal.render_frame(&frame, 80).unwrap();

        let mut terminal2 = terminal;
        terminal2.writer.bytes.clear();
        terminal2.render_frame(&frame, 80).unwrap();
        let output = String::from_utf8_lossy(&terminal2.writer.bytes);
        assert!(!output.contains("hello"));
        assert!(!output.contains("world"));
    }

    #[test]
    fn changing_middle_line_rewrites_from_diff() {
        let mut terminal = TerminalScreen::new(FakeWriter::new());
        let frame1 = frame(&["aaa", "bbb", "ccc"], 80, 24);
        terminal.render_frame(&frame1, 80).unwrap();

        terminal.writer.bytes.clear();
        let frame2 = frame(&["aaa", "BBB", "ccc"], 80, 24);
        terminal.render_frame(&frame2, 80).unwrap();
        let output = String::from_utf8_lossy(&terminal.writer.bytes);
        assert!(output.contains("BBB"));
        assert!(output.contains("ccc"));
    }

    #[test]
    fn push_to_scrollback_clears_prev_frame() {
        let mut terminal = TerminalScreen::new(FakeWriter::new());

        let frame = frame(&["managed line"], 80, 24);
        terminal.render_frame(&frame, 80).unwrap();

        terminal
            .push_to_scrollback(&[Line::new("scrolled")], 80)
            .unwrap();

        terminal.writer.bytes.clear();
        terminal.render_frame(&frame, 80).unwrap();
        let output = String::from_utf8_lossy(&terminal.writer.bytes);
        assert!(output.contains("managed line"));
    }

    #[test]
    fn push_to_scrollback_empty_is_noop() {
        let mut terminal = TerminalScreen::new(FakeWriter::new());
        terminal.push_to_scrollback(&[], 80).unwrap();
        assert!(terminal.writer.bytes.is_empty());
    }

    #[test]
    fn width_change_forces_full_rerender() {
        let mut terminal = TerminalScreen::new(FakeWriter::new());
        let frame = frame(&["a", "b"], 80, 24);
        terminal.render_frame(&frame, 80).unwrap();

        terminal.writer.bytes.clear();
        terminal.render_frame(&frame, 120).unwrap();
        let output = String::from_utf8_lossy(&terminal.writer.bytes);
        assert!(output.contains('a'));
        assert!(output.contains('b'));
    }
}
