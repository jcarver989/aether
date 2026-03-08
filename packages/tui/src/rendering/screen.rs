use crossterm::{
    QueueableCommand,
    cursor::MoveUp,
    terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate},
};
use std::io::{self, Write};

use super::line::Line;

/// Relative-cursor screen renderer.
///
/// Uses relative cursor movement (`MoveUp` + `\r`) to navigate back to the
/// start of the managed region. This avoids absolute row tracking, which breaks
/// when the terminal scrolls content upward.
///
/// **Cursor invariant:** After every `render` or `push_to_scrollback`, the
/// cursor sits at the end of the last managed line.
pub struct Screen {
    prev_frame: Vec<Line>,
    last_width: u16,
}

impl Screen {
    pub fn new() -> Self {
        Self {
            prev_frame: Vec::new(),
            last_width: 0,
        }
    }

    /// Render `new_frame`, replacing only the changed portion of the managed region.
    /// Returns the number of lines written.
    ///
    /// When `width` changes from the previous call, the previous frame is discarded
    /// to force a full re-render (line content depends on terminal width).
    pub fn render<W: Write>(
        &mut self,
        new_frame: &[Line],
        width: u16,
        writer: &mut W,
    ) -> io::Result<usize> {
        // Remember actual on-screen line count before any clear, since the
        // cursor position still reflects the previously rendered frame.
        let prev_on_screen = self.prev_frame.len();

        if width != self.last_width {
            self.prev_frame.clear();
            self.last_width = width;
        }

        if new_frame == self.prev_frame {
            return Ok(0);
        }

        writer.queue(BeginSynchronizedUpdate)?;

        // Find first line that differs between old and new
        let first_diff = self
            .prev_frame
            .iter()
            .zip(new_frame.iter())
            .position(|(old, new)| old != new)
            .unwrap_or(self.prev_frame.len().min(new_frame.len()));

        // Clamp so we always rewrite at least the last line of new_frame,
        // ensuring the cursor ends at the correct position when the frame shrinks.
        let rewrite_from = if new_frame.is_empty() {
            0
        } else {
            first_diff.min(new_frame.len() - 1)
        };

        // Position cursor at the start of the rewrite_from line.
        // Use prev_on_screen (not prev_frame.len()) because the cursor is
        // still at the end of whatever was last rendered, even if prev_frame
        // was cleared by a width change.
        if rewrite_from < prev_on_screen {
            let lines_up = prev_on_screen - 1 - rewrite_from;
            if lines_up > 0 {
                writer.queue(MoveUp(u16::try_from(lines_up).unwrap_or(u16::MAX)))?;
            }
            write!(writer, "\r")?;
            writer.queue(Clear(ClearType::FromCursorDown))?;
        } else if prev_on_screen > 0 {
            // Appending past the end of the previous frame
            write!(writer, "\r\n")?;
            writer.queue(Clear(ClearType::FromCursorDown))?;
        }

        // Write new_frame[rewrite_from..]
        let to_write = &new_frame[rewrite_from..];
        for (i, line) in to_write.iter().enumerate() {
            write!(writer, "{}", line.to_ansi_string())?;
            if i < to_write.len() - 1 {
                write!(writer, "\r\n")?;
            }
        }

        writer.queue(EndSynchronizedUpdate)?;
        writer.flush()?;
        let lines_written = to_write.len();
        self.prev_frame = new_frame.to_vec();
        Ok(lines_written)
    }

    /// Flush pre-wrapped visual lines to scrollback, clearing `prev_frame`.
    ///
    /// Moves to the start of the managed region, clears it, writes the
    /// scrollback lines with `\r\n` so they become permanent, then clears
    /// `prev_frame`. The cursor ends on the line after the last scrollback
    /// line.
    pub fn push_to_scrollback<W: Write>(
        &mut self,
        visual_lines: &[Line],
        writer: &mut W,
    ) -> io::Result<()> {
        if visual_lines.is_empty() {
            return Ok(());
        }

        writer.queue(BeginSynchronizedUpdate)?;

        // Move cursor to column 0 of the first managed line
        if self.prev_frame.len() > 1 {
            writer.queue(MoveUp(
                u16::try_from(self.prev_frame.len() - 1).unwrap_or(u16::MAX),
            ))?;
        }
        write!(writer, "\r")?;

        // Clear everything from here to end of screen
        writer.queue(Clear(ClearType::FromCursorDown))?;

        // Write scrollback lines (permanent, with \r\n)
        for line in visual_lines {
            write!(writer, "{}\r\n", line.to_ansi_string())?;
        }

        writer.queue(EndSynchronizedUpdate)?;
        writer.flush()?;

        self.prev_frame.clear();
        Ok(())
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn empty_to_empty_is_noop() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let written = screen.render(&[], 80, &mut w).unwrap();
        assert_eq!(written, 0);
        assert!(w.bytes.is_empty());
    }

    #[test]
    fn first_render_writes_all_lines() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame = vec![Line::new("hello"), Line::new("world")];
        let written = screen.render(&frame, 80, &mut w).unwrap();
        assert_eq!(written, 2);
        let output = String::from_utf8_lossy(&w.bytes);
        assert!(output.contains("hello"));
        assert!(output.contains("world"));
    }

    #[test]
    fn identical_frames_produce_no_writes() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame = vec![Line::new("hello"), Line::new("world")];
        screen.render(&frame, 80, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let written = screen.render(&frame, 80, &mut w2).unwrap();
        assert_eq!(written, 0);
        assert!(w2.bytes.is_empty());
    }

    #[test]
    fn changing_middle_line_rewrites_from_diff() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("aaa"), Line::new("bbb"), Line::new("ccc")];
        screen.render(&frame1, 80, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("aaa"), Line::new("BBB"), Line::new("ccc")];
        let written = screen.render(&frame2, 80, &mut w2).unwrap();
        // Differential: rewrites from line 1 onward (2 lines)
        assert_eq!(written, 2);
        let output = String::from_utf8_lossy(&w2.bytes);
        assert!(output.contains("BBB"));
        assert!(output.contains("ccc"));
    }

    #[test]
    fn shrinking_frame_clears_leftover_rows() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        screen.render(&frame1, 80, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("a")];
        let written = screen.render(&frame2, 80, &mut w2).unwrap();
        // Rewrites 1 line (clear from cursor down handles the rest)
        assert_eq!(written, 1);
    }

    #[test]
    fn growing_frame_writes_only_new_lines() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("a")];
        screen.render(&frame1, 80, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        let written = screen.render(&frame2, 80, &mut w2).unwrap();
        // Differential: first line matches, writes only 2 new lines
        assert_eq!(written, 2);
        let output = String::from_utf8_lossy(&w2.bytes);
        assert!(output.contains('b'));
        assert!(output.contains('c'));
    }

    #[test]
    fn appending_lines_only_writes_new_ones() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("a"), Line::new("b")];
        screen.render(&frame1, 80, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        let written = screen.render(&frame2, 80, &mut w2).unwrap();
        // Only the appended line is written
        assert_eq!(written, 1);
    }

    #[test]
    fn only_last_line_changed() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        screen.render(&frame1, 80, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("a"), Line::new("b"), Line::new("X")];
        let written = screen.render(&frame2, 80, &mut w2).unwrap();
        // Only the last changed line is written
        assert_eq!(written, 1);
    }

    #[test]
    fn push_to_scrollback_clears_prev_frame() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();

        let frame = vec![Line::new("managed line")];
        screen.render(&frame, 80, &mut w).unwrap();

        screen
            .push_to_scrollback(&[Line::new("scrolled")], &mut w)
            .unwrap();

        // After push_to_scrollback, prev_frame is cleared. Rendering the same
        // frame again should produce writes (full re-render, not a no-op diff).
        let mut w2 = FakeWriter::new();
        let written = screen.render(&frame, 80, &mut w2).unwrap();
        assert!(written > 0, "expected re-render after scrollback push");
    }

    #[test]
    fn push_to_scrollback_empty_is_noop() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        screen.push_to_scrollback(&[], &mut w).unwrap();
        assert!(w.bytes.is_empty());
    }

    #[test]
    fn width_change_forces_full_rerender() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame = vec![Line::new("a"), Line::new("b")];
        screen.render(&frame, 80, &mut w).unwrap();

        // Same frame but different width → full re-render
        let mut w2 = FakeWriter::new();
        let written = screen.render(&frame, 120, &mut w2).unwrap();
        assert_eq!(written, 2);
    }
}
