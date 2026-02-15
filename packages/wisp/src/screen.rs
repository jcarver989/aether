use crossterm::{QueueableCommand, cursor::MoveTo, terminal::{Clear, ClearType}};
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

/// Virtual-diff screen renderer.
///
/// Holds a virtual buffer of what was last drawn. On each `render()` call,
/// diffs the new frame against the previous one and only rewrites from the
/// first changed line downward. Writes directly via crossterm's `QueueableCommand`.
pub struct Screen {
    prev_frame: Vec<Line>,
    origin_row: u16,
}

impl Screen {
    pub fn new(origin_row: u16) -> Self {
        Self {
            prev_frame: Vec::new(),
            origin_row,
        }
    }

    /// Diff `new_frame` against the previous frame and write only the changed
    /// portion. Returns the number of lines written.
    pub fn render<W: Write>(&mut self, new_frame: &[Line], writer: &mut W) -> io::Result<usize> {
        let first_diff = self
            .prev_frame
            .iter()
            .zip(new_frame.iter())
            .position(|(old, new)| old != new)
            .unwrap_or_else(|| {
                // No mismatch among the overlap — diff starts where lengths diverge
                self.prev_frame.len().min(new_frame.len())
            });

        // Nothing changed and same length — no-op
        if first_diff >= new_frame.len() && new_frame.len() == self.prev_frame.len() {
            return Ok(0);
        }

        let mut lines_written = 0;

        // Write changed lines
        for (i, line) in new_frame.iter().enumerate().skip(first_diff) {
            let row = self.origin_row + i as u16;
            writer.queue(MoveTo(0, row))?;
            writer.queue(Clear(ClearType::CurrentLine))?;
            write!(writer, "{line}")?;
            lines_written += 1;
        }

        // Clear leftover rows if old frame was longer
        for i in new_frame.len()..self.prev_frame.len() {
            let row = self.origin_row + i as u16;
            writer.queue(MoveTo(0, row))?;
            writer.queue(Clear(ClearType::CurrentLine))?;
            lines_written += 1;
        }

        writer.flush()?;
        self.prev_frame = new_frame.to_vec();
        Ok(lines_written)
    }

    /// Commit lines above the managed region to scrollback.
    ///
    /// Moves the cursor below the managed region and prints each line with a
    /// newline so the terminal scrolls them into its native scrollback buffer.
    /// Then resets `origin_row` to the current cursor position and clears the
    /// previous frame.
    pub fn push_to_scrollback<W: Write>(
        &mut self,
        lines: &[Line],
        writer: &mut W,
    ) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }

        // Clear the current managed region
        for i in 0..self.prev_frame.len() {
            let row = self.origin_row + i as u16;
            writer.queue(MoveTo(0, row))?;
            writer.queue(Clear(ClearType::CurrentLine))?;
        }

        // Write scrollback lines starting at origin
        writer.queue(MoveTo(0, self.origin_row))?;
        for line in lines {
            write!(writer, "{line}\r\n")?;
        }
        writer.flush()?;

        // New origin is after the scrollback lines
        self.origin_row += lines.len() as u16;
        self.prev_frame.clear();
        Ok(())
    }

    #[allow(dead_code)]
    pub fn origin_row(&self) -> u16 {
        self.origin_row
    }

    #[allow(dead_code)]
    pub fn set_origin_row(&mut self, row: u16) {
        self.origin_row = row;
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
        let mut screen = Screen::new(0);
        let mut w = FakeWriter::new();
        let written = screen.render(&[], &mut w).unwrap();
        assert_eq!(written, 0);
        assert!(w.bytes.is_empty());
    }

    #[test]
    fn first_render_writes_all_lines() {
        let mut screen = Screen::new(0);
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
        let mut screen = Screen::new(0);
        let mut w = FakeWriter::new();
        let frame = vec![Line::new("hello"), Line::new("world")];
        screen.render(&frame, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let written = screen.render(&frame, &mut w2).unwrap();
        assert_eq!(written, 0);
        assert!(w2.bytes.is_empty());
    }

    #[test]
    fn changing_second_line_only_rewrites_from_there() {
        let mut screen = Screen::new(0);
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("aaa"), Line::new("bbb"), Line::new("ccc")];
        screen.render(&frame1, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("aaa"), Line::new("BBB"), Line::new("ccc")];
        let written = screen.render(&frame2, &mut w2).unwrap();
        // Should rewrite lines 1 and 2 (from first diff to end)
        assert_eq!(written, 2);
        let output = String::from_utf8_lossy(&w2.bytes);
        assert!(output.contains("BBB"));
        assert!(output.contains("ccc"));
        // Should NOT contain "aaa" since line 0 didn't change
        assert!(!output.contains("aaa"));
    }

    #[test]
    fn shrinking_frame_clears_leftover_rows() {
        let mut screen = Screen::new(0);
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        screen.render(&frame1, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("a")];
        let written = screen.render(&frame2, &mut w2).unwrap();
        // Lines 1 and 2 need clearing (old frame had 3 lines, new has 1)
        assert_eq!(written, 2);
    }

    #[test]
    fn growing_frame_writes_new_lines() {
        let mut screen = Screen::new(0);
        let mut w = FakeWriter::new();
        let frame1 = vec![Line::new("a")];
        screen.render(&frame1, &mut w).unwrap();

        let mut w2 = FakeWriter::new();
        let frame2 = vec![Line::new("a"), Line::new("b"), Line::new("c")];
        let written = screen.render(&frame2, &mut w2).unwrap();
        assert_eq!(written, 2);
        let output = String::from_utf8_lossy(&w2.bytes);
        assert!(output.contains("b"));
        assert!(output.contains("c"));
        assert!(!output.contains("a"));
    }

    #[test]
    fn origin_row_offsets_cursor_positions() {
        let mut screen = Screen::new(5);
        let mut w = FakeWriter::new();
        let frame = vec![Line::new("hi")];
        screen.render(&frame, &mut w).unwrap();
        // The output should contain a MoveTo(0, 5) ANSI sequence
        let output = String::from_utf8_lossy(&w.bytes);
        // MoveTo(0,5) in ANSI is ESC[6;1H (1-indexed)
        assert!(output.contains("\x1b[6;1H"), "expected MoveTo(0,5), got: {output}");
    }

    #[test]
    fn push_to_scrollback_updates_origin() {
        let mut screen = Screen::new(0);
        let mut w = FakeWriter::new();

        let frame = vec![Line::new("managed line")];
        screen.render(&frame, &mut w).unwrap();

        screen
            .push_to_scrollback(&[Line::new("scrolled")], &mut w)
            .unwrap();

        // Origin was 0, wrote 1 scrollback line → new origin = 1
        assert_eq!(screen.origin_row(), 1);
        assert!(screen.prev_frame().is_empty());
    }

    #[test]
    fn push_to_scrollback_empty_is_noop() {
        let mut screen = Screen::new(3);
        let mut w = FakeWriter::new();
        screen.push_to_scrollback(&[], &mut w).unwrap();
        assert_eq!(screen.origin_row(), 3);
    }

    #[test]
    fn prev_frame_tracks_last_render() {
        let mut screen = Screen::new(0);
        let mut w = FakeWriter::new();
        let frame = vec![Line::new("x"), Line::new("y")];
        screen.render(&frame, &mut w).unwrap();
        assert_eq!(screen.prev_frame(), &frame);
    }
}
