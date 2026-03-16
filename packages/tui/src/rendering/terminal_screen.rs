use crossterm::QueueableCommand;
use crossterm::cursor::{Hide, MoveDown, MoveRight, MoveTo, MoveUp, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate};
use std::io::{self, Write};

use super::line::Line;

pub(crate) enum TerminalCommand<'a> {
    ClearAll,
    SetCursorVisible(bool),
    SetMouseCapture(bool),
    RestoreCursorPosition,
    PlaceCursor {
        rows_up: u16,
        col: u16,
    },
    RewriteVisibleLines {
        rows_up: u16,
        append_after_existing: bool,
        lines: &'a [Line],
    },
    PushScrollbackLines {
        previous_visible_rows: usize,
        lines: &'a [Line],
    },
}

pub(crate) struct TerminalScreen<W: Write> {
    pub(super) writer: W,
    cursor_row_offset: u16,
    cursor_visible: bool,
    mouse_captured: bool,
}

impl<W: Write> TerminalScreen<W> {
    pub(crate) fn new(writer: W) -> Self {
        Self {
            writer,
            cursor_row_offset: 0,
            cursor_visible: true,
            mouse_captured: false,
        }
    }

    pub(crate) fn execute_batch(&mut self, commands: &[TerminalCommand<'_>]) -> io::Result<()> {
        self.writer.queue(BeginSynchronizedUpdate)?;
        for command in commands {
            self.execute(command)?;
        }
        self.writer.queue(EndSynchronizedUpdate)?;
        self.writer.flush()
    }

    pub(crate) fn execute(&mut self, command: &TerminalCommand<'_>) -> io::Result<()> {
        match command {
            TerminalCommand::ClearAll => {
                self.writer.queue(Clear(ClearType::All))?;
                self.writer.queue(Clear(ClearType::Purge))?;
                self.writer.queue(MoveTo(0, 0))?;
                self.cursor_row_offset = 0;
            }
            TerminalCommand::SetCursorVisible(visible) => {
                if *visible != self.cursor_visible {
                    if *visible {
                        self.writer.queue(Show)?;
                    } else {
                        self.writer.queue(Hide)?;
                    }
                    self.cursor_visible = *visible;
                }
            }
            TerminalCommand::SetMouseCapture(enable) => {
                if *enable != self.mouse_captured {
                    if *enable {
                        self.writer.queue(EnableMouseCapture)?;
                    } else {
                        self.writer.queue(DisableMouseCapture)?;
                    }
                    self.mouse_captured = *enable;
                }
            }
            TerminalCommand::PlaceCursor { rows_up, col } => {
                self.writer.queue(MoveUp(*rows_up))?;
                write!(self.writer, "\r")?;
                if *col > 0 {
                    self.writer.queue(MoveRight(*col))?;
                }
                self.cursor_row_offset = *rows_up;
            }
            TerminalCommand::RestoreCursorPosition => {
                if self.cursor_row_offset > 0 {
                    self.writer.queue(MoveDown(self.cursor_row_offset))?;
                    self.cursor_row_offset = 0;
                }
            }
            TerminalCommand::RewriteVisibleLines {
                rows_up,
                append_after_existing,
                lines,
            } => {
                if *rows_up > 0 {
                    self.writer.queue(MoveUp(*rows_up))?;
                    write!(self.writer, "\r")?;
                } else if *append_after_existing {
                    write!(self.writer, "\r\n")?;
                } else {
                    write!(self.writer, "\r")?;
                }
                self.writer.queue(Clear(ClearType::FromCursorDown))?;
                for (i, line) in lines.iter().enumerate() {
                    write!(self.writer, "{}", line.to_ansi_string())?;
                    if i < lines.len() - 1 {
                        write!(self.writer, "\r\n")?;
                    }
                }
            }
            TerminalCommand::PushScrollbackLines {
                previous_visible_rows,
                lines,
            } => {
                if *previous_visible_rows > 1 {
                    let rows_up = u16::try_from(previous_visible_rows - 1).unwrap_or(u16::MAX);
                    self.writer.queue(MoveUp(rows_up))?;
                }
                write!(self.writer, "\r")?;
                self.writer.queue(Clear(ClearType::FromCursorDown))?;
                for line in *lines {
                    write!(self.writer, "{}", line.to_ansi_string())?;
                    write!(self.writer, "\r\n")?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn reset_cursor_offset(&mut self) {
        self.cursor_row_offset = 0;
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

        fn output(&self) -> String {
            String::from_utf8_lossy(&self.bytes).into_owned()
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
    fn clear_all_emits_expected_sequences() {
        let mut screen = TerminalScreen::new(FakeWriter::new());
        screen.execute_batch(&[TerminalCommand::ClearAll]).unwrap();
        let output = screen.writer.output();
        assert!(output.contains("\x1b[2J"), "missing Clear(All)");
        assert!(output.contains("\x1b[3J"), "missing Clear(Purge)");
        assert!(output.contains("\x1b[1;1H"), "missing MoveTo(0,0)");
    }

    #[test]
    fn set_cursor_visible_only_writes_on_state_change() {
        let mut screen = TerminalScreen::new(FakeWriter::new());

        // Initially visible, setting visible again should be a no-op
        // But execute() wraps in sync, so we need to check the content between sync markers
        screen
            .execute_batch(&[TerminalCommand::SetCursorVisible(true)])
            .unwrap();
        // Extract content between sync markers
        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);
        assert!(content.is_empty(), "should have no content for no-op");

        // Hide cursor
        screen.writer.bytes.clear();
        screen
            .execute_batch(&[TerminalCommand::SetCursorVisible(false)])
            .unwrap();
        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);
        assert!(content.contains("\x1b[?25l"), "missing Hide");

        // Hide again should be a no-op
        screen.writer.bytes.clear();
        screen
            .execute_batch(&[TerminalCommand::SetCursorVisible(false)])
            .unwrap();
        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);
        assert!(content.is_empty(), "should have no content for no-op");

        // Show cursor
        screen.writer.bytes.clear();
        screen
            .execute_batch(&[TerminalCommand::SetCursorVisible(true)])
            .unwrap();
        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);
        assert!(content.contains("\x1b[?25h"), "missing Show");
    }

    #[test]
    fn place_cursor_and_restore_round_trip() {
        let mut screen = TerminalScreen::new(FakeWriter::new());

        screen
            .execute_batch(&[TerminalCommand::PlaceCursor { rows_up: 3, col: 5 }])
            .unwrap();
        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);
        assert!(content.contains("\x1b[3A"), "missing MoveUp(3)");
        assert!(content.contains("\r"), "missing carriage return");
        assert!(content.contains("\x1b[5C"), "missing MoveRight(5)");

        screen.writer.bytes.clear();
        screen
            .execute_batch(&[TerminalCommand::RestoreCursorPosition])
            .unwrap();
        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);
        assert!(content.contains("\x1b[3B"), "missing MoveDown(3)");

        // Restore again should be a no-op (offset is 0)
        screen.writer.bytes.clear();
        screen
            .execute_batch(&[TerminalCommand::RestoreCursorPosition])
            .unwrap();
        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);
        assert!(
            content.is_empty(),
            "should have no content for no-op restore"
        );
    }

    #[test]
    fn execute_wraps_commands_in_synchronized_update() {
        let mut screen = TerminalScreen::new(FakeWriter::new());
        screen.execute_batch(&[TerminalCommand::ClearAll]).unwrap();
        let output = screen.writer.output();
        let begin = output
            .find("\x1b[?2026h")
            .expect("missing BeginSynchronizedUpdate");
        let clear = output.find("\x1b[2J").expect("missing Clear(All)");
        let end = output
            .find("\x1b[?2026l")
            .expect("missing EndSynchronizedUpdate");
        assert!(begin < clear, "Begin should come before content");
        assert!(clear < end, "Content should come before End");
    }

    #[test]
    fn rewrite_visible_lines_emits_expected_sequences() {
        let mut screen = TerminalScreen::new(FakeWriter::new());
        let lines = vec![Line::new("line1"), Line::new("line2"), Line::new("line3")];

        screen
            .execute_batch(&[TerminalCommand::RewriteVisibleLines {
                rows_up: 2,
                append_after_existing: false,
                lines: &lines,
            }])
            .unwrap();

        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);

        // Should move up
        assert!(content.contains("\x1b[2A"), "missing MoveUp(2)");

        // Should have carriage return
        assert!(content.contains("\r"), "missing carriage return");

        // Should clear below
        assert!(content.contains("\x1b[J"), "missing Clear(FromCursorDown)");

        // Should write all lines
        assert!(content.contains("line1"), "missing line1");
        assert!(content.contains("line2"), "missing line2");
        assert!(content.contains("line3"), "missing line3");

        // Should have newlines between lines
        let line1_pos = content.find("line1").unwrap();
        let line2_pos = content.find("line2").unwrap();
        let line3_pos = content.find("line3").unwrap();

        // There should be a newline between line1 and line2
        let between_1_and_2 = &content[line1_pos + 5..line2_pos];
        assert!(
            between_1_and_2.contains("\r\n"),
            "missing newline between line1 and line2"
        );

        // There should be a newline between line2 and line3
        let between_2_and_3 = &content[line2_pos + 5..line3_pos];
        assert!(
            between_2_and_3.contains("\r\n"),
            "missing newline between line2 and line3"
        );

        // There should NOT be a trailing newline after line3
        let after_line3 = &content[line3_pos + 5..];
        assert!(
            !after_line3.starts_with("\r\n"),
            "should not have trailing newline"
        );
    }

    #[test]
    fn rewrite_visible_lines_with_zero_rows_up() {
        let mut screen = TerminalScreen::new(FakeWriter::new());
        let lines = vec![Line::new("only")];

        screen
            .execute_batch(&[TerminalCommand::RewriteVisibleLines {
                rows_up: 0,
                append_after_existing: false,
                lines: &lines,
            }])
            .unwrap();

        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);

        // Should NOT move up when rows_up is 0
        assert!(
            !content.contains("\x1b[1A")
                && !content.contains("\x1b[2A")
                && !content.contains("\x1b[3A"),
            "should not have MoveUp when rows_up is 0, got: {:?}",
            content
        );
        assert!(content.contains("only"), "missing content");
    }

    #[test]
    fn rewrite_visible_lines_append_moves_to_next_row_before_clearing() {
        let mut screen = TerminalScreen::new(FakeWriter::new());
        let lines = vec![Line::new("appended")];

        screen
            .execute_batch(&[TerminalCommand::RewriteVisibleLines {
                rows_up: 0,
                append_after_existing: true,
                lines: &lines,
            }])
            .unwrap();

        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);
        assert!(
            content.starts_with("\r\n\x1b[Jappended"),
            "append case should move to next row before clearing, got: {:?}",
            content
        );
    }

    #[test]
    fn push_scrollback_lines_emits_expected_sequences() {
        let mut screen = TerminalScreen::new(FakeWriter::new());
        let lines = vec![Line::new("scroll1"), Line::new("scroll2")];

        screen
            .execute_batch(&[TerminalCommand::PushScrollbackLines {
                previous_visible_rows: 4,
                lines: &lines,
            }])
            .unwrap();

        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);

        // Should move up (previous_visible_rows - 1 = 3)
        assert!(content.contains("\x1b[3A"), "missing MoveUp(3)");

        // Should have carriage return
        assert!(content.contains("\r"), "missing carriage return");

        // Should clear below
        assert!(content.contains("\x1b[J"), "missing Clear(FromCursorDown)");

        // Should write all lines
        assert!(content.contains("scroll1"), "missing scroll1");
        assert!(content.contains("scroll2"), "missing scroll2");

        // Every line should be followed by newline
        assert!(
            content.ends_with("\r\n"),
            "content should end with newline, got: {:?}",
            content
        );
    }

    #[test]
    fn push_scrollback_lines_with_one_previous_row() {
        let mut screen = TerminalScreen::new(FakeWriter::new());
        let lines = vec![Line::new("scroll")];

        screen
            .execute_batch(&[TerminalCommand::PushScrollbackLines {
                previous_visible_rows: 1,
                lines: &lines,
            }])
            .unwrap();

        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);

        // Should NOT move up when previous_visible_rows is 1
        assert!(
            !content.contains("\x1b[1A"),
            "should not move up when previous_visible_rows is 1"
        );
        assert!(
            !content.contains("\x1b[2A") && !content.contains("\x1b[3A"),
            "should not have movement escapes"
        );
    }

    #[test]
    fn push_scrollback_lines_with_zero_previous_rows() {
        let mut screen = TerminalScreen::new(FakeWriter::new());
        let lines = vec![Line::new("scroll")];

        screen
            .execute_batch(&[TerminalCommand::PushScrollbackLines {
                previous_visible_rows: 0,
                lines: &lines,
            }])
            .unwrap();

        let output = screen.writer.output();
        let content = extract_content_between_sync(&output);

        // Should NOT move up when previous_visible_rows is 0 or 1
        // (We still emit carriage return, clear, and content - just no MoveUp)
        assert!(
            !content.contains("\x1b[1A")
                && !content.contains("\x1b[2A")
                && !content.contains("\x1b[3A")
                && !content.contains("\x1b[4A"),
            "should not have MoveUp when previous_visible_rows is 0, got: {:?}",
            content
        );
        // But we should still have the content
        assert!(content.contains("scroll"), "missing content");
        assert!(content.contains("\r\n"), "should have newlines");
    }

    fn extract_content_between_sync(output: &str) -> String {
        let begin_tag = "\x1b[?2026h";
        let end_tag = "\x1b[?2026l";

        if let (Some(begin), Some(end)) = (output.find(begin_tag), output.find(end_tag)) {
            let start = begin + begin_tag.len();
            if start <= end {
                return output[start..end].to_string();
            }
        }
        output.to_string()
    }
}
