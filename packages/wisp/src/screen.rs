use crossterm::{QueueableCommand, cursor::MoveUp, terminal::{Clear, ClearType}};
use std::io::{self, Write};

/// A single line of pre-formatted terminal output.
/// May contain ANSI color codes. Equality is byte-identical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line(String);

impl Line {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

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
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen {
    pub fn new() -> Self {
        Self {
            prev_frame: Vec::new(),
        }
    }

    /// Render `new_frame`, replacing the previous managed region in-place.
    /// Returns the number of lines written.
    pub fn render<W: Write>(&mut self, new_frame: &[Line], writer: &mut W) -> io::Result<usize> {
        if new_frame == self.prev_frame {
            return Ok(0);
        }

        // Move cursor to column 0 of the first managed line
        if self.prev_frame.len() > 1 {
            writer.queue(MoveUp((self.prev_frame.len() - 1) as u16))?;
        }
        write!(writer, "\r")?;

        // Clear everything from here to end of screen
        writer.queue(Clear(ClearType::FromCursorDown))?;

        // Write all new frame lines
        for (i, line) in new_frame.iter().enumerate() {
            write!(writer, "{line}")?;
            if i < new_frame.len() - 1 {
                write!(writer, "\r\n")?;
            }
        }

        writer.flush()?;
        let lines_written = new_frame.len();
        self.prev_frame = new_frame.to_vec();
        Ok(lines_written)
    }

    /// Commit lines to scrollback, replacing the current managed region.
    ///
    /// Moves to the start of the managed region, clears it, writes the
    /// scrollback lines with `\r\n` so they become permanent, then clears
    /// `prev_frame`. The cursor ends on the line after the last scrollback
    /// line.
    pub fn push_to_scrollback<W: Write>(
        &mut self,
        lines: &[Line],
        writer: &mut W,
    ) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }

        // Move cursor to column 0 of the first managed line
        if self.prev_frame.len() > 1 {
            writer.queue(MoveUp((self.prev_frame.len() - 1) as u16))?;
        }
        write!(writer, "\r")?;

        // Clear everything from here to end of screen
        writer.queue(Clear(ClearType::FromCursorDown))?;

        // Write scrollback lines (permanent, with \r\n)
        for line in lines {
            write!(writer, "{line}\r\n")?;
        }

        writer.flush()?;
        self.prev_frame.clear();
        Ok(())
    }

    #[allow(dead_code)]
    pub fn prev_frame(&self) -> &[Line] {
        &self.prev_frame
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
        let written = screen.render(&[], &mut w).unwrap();
        assert_eq!(written, 0);
        assert!(w.bytes.is_empty());
    }

    #[test]
    fn first_render_writes_all_lines() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame = vec![Line::new("hello"), Line::new("world")];
        let written = screen.render(&frame, &mut w).unwrap();
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
        screen.render(&frame, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let written = screen.render(&frame, &mut w2).unwrap();
        assert_eq!(written, 0);
        assert!(w2.bytes.is_empty());
    }

    #[test]
    fn changing_line_rewrites_entire_frame() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("aaa"), Line::new("bbb"), Line::new("ccc")];
        screen.render(&frame1, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("aaa"), Line::new("BBB"), Line::new("ccc")];
        let written = screen.render(&frame2, &mut w2).unwrap();
        // Now rewrites all lines since we use clear-and-rewrite
        assert_eq!(written, 3);
        let output = String::from_utf8_lossy(&w2.bytes);
        assert!(output.contains("BBB"));
        assert!(output.contains("ccc"));
    }

    #[test]
    fn shrinking_frame_clears_leftover_rows() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        screen.render(&frame1, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("a")];
        let written = screen.render(&frame2, &mut w2).unwrap();
        // Rewrites 1 line (clear from cursor down handles the rest)
        assert_eq!(written, 1);
    }

    #[test]
    fn growing_frame_writes_new_lines() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("a")];
        screen.render(&frame1, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        let written = screen.render(&frame2, &mut w2).unwrap();
        assert_eq!(written, 3);
        let output = String::from_utf8_lossy(&w2.bytes);
        assert!(output.contains("b"));
        assert!(output.contains("c"));
    }

    #[test]
    fn push_to_scrollback_clears_prev_frame() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();

        let frame = vec![Line::new("managed line")];
        screen.render(&frame, &mut w).unwrap();

        screen
            .push_to_scrollback(&[Line::new("scrolled")], &mut w)
            .unwrap();

        assert!(screen.prev_frame().is_empty());
    }

    #[test]
    fn push_to_scrollback_empty_is_noop() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        screen.push_to_scrollback(&[], &mut w).unwrap();
        assert!(w.bytes.is_empty());
    }

    #[test]
    fn prev_frame_tracks_last_render() {
        let mut screen = Screen::new();
        let mut w = FakeWriter::new();
        let frame = vec![Line::new("x"), Line::new("y")];
        screen.render(&frame, &mut w).unwrap();
        assert_eq!(screen.prev_frame(), &frame);
    }
}
