use std::io::{self, Write};

/// A virtual terminal buffer for testing terminal output.
/// Captures all writes, tracks cursor position, and parses ANSI escape sequences.
///
/// Implements delayed wrapping (DEC-style): when the cursor reaches the last
/// column, it stays there with a pending-wrap flag. The next printable character
/// triggers the wrap to column 0 of the next line. A `\r` clears the flag.
#[derive(Debug, Clone)]
pub struct TestTerminal {
    /// 2D buffer of characters (row, column)
    buffer: Vec<Vec<char>>,
    /// Current cursor position (column, row)
    cursor: (u16, u16),
    /// Saved cursor position (for save/restore)
    saved_cursor: Option<(u16, u16)>,
    /// Terminal size (columns, rows)
    size: (u16, u16),
    /// Buffer for incomplete escape sequences
    escape_buffer: Vec<u8>,
    /// Delayed wrap: cursor hit last column but hasn't wrapped yet
    pending_wrap: bool,
}

impl TestTerminal {
    /// Create a new test terminal with given size
    pub fn new(columns: u16, rows: u16) -> Self {
        let buffer = vec![vec![' '; columns as usize]; rows as usize];
        Self {
            buffer,
            cursor: (0, 0),
            saved_cursor: None,
            size: (columns, rows),
            escape_buffer: Vec::new(),
            pending_wrap: false,
        }
    }

    /// Get all lines as a vector of strings (trailing whitespace trimmed)
    pub fn get_lines(&self) -> Vec<String> {
        self.buffer
            .iter()
            .map(|chars| chars.iter().collect::<String>().trim_end().to_string())
            .collect()
    }

    /// Get current cursor position as (column, row).
    #[allow(dead_code)]
    pub fn cursor_position(&self) -> (u16, u16) {
        self.cursor
    }

    /// Clear the entire buffer
    pub fn clear(&mut self) {
        for row in &mut self.buffer {
            for ch in row {
                *ch = ' ';
            }
        }
    }

    /// Clear the current line
    pub fn clear_line(&mut self) {
        if let Some(row) = self.buffer.get_mut(self.cursor.1 as usize) {
            for ch in row {
                *ch = ' ';
            }
        }
    }

    /// Move cursor to absolute position
    pub fn move_to(&mut self, col: u16, row: u16) {
        self.cursor = (
            col.min(self.size.0.saturating_sub(1)),
            row.min(self.size.1.saturating_sub(1)),
        );
        self.pending_wrap = false;
    }

    /// Move cursor to column (keep same row)
    pub fn move_to_column(&mut self, col: u16) {
        self.cursor.0 = col.min(self.size.0.saturating_sub(1));
        self.pending_wrap = false;
    }

    /// Move cursor left by n positions
    pub fn move_left(&mut self, n: u16) {
        self.cursor.0 = self.cursor.0.saturating_sub(n);
        self.pending_wrap = false;
    }

    /// Move cursor right by n positions
    pub fn move_right(&mut self, n: u16) {
        self.cursor.0 = (self.cursor.0 + n).min(self.size.0.saturating_sub(1));
        self.pending_wrap = false;
    }

    /// Write a single character at current cursor position and advance cursor
    fn write_char(&mut self, ch: char) {
        match ch {
            '\n' => {
                self.pending_wrap = false;
                if self.cursor.1 >= self.size.1.saturating_sub(1) {
                    // At last row: scroll buffer up by 1
                    self.buffer.remove(0);
                    self.buffer.push(vec![' '; self.size.0 as usize]);
                    // Cursor stays at last row
                } else {
                    self.cursor.1 += 1;
                }
                self.cursor.0 = 0;
            }
            '\r' => {
                // Move to column 0, clear pending wrap
                self.cursor.0 = 0;
                self.pending_wrap = false;
            }
            '\t' => {
                // Tab = 4 spaces
                for _ in 0..4 {
                    self.write_char_at_cursor(' ');
                }
            }
            _ => {
                self.write_char_at_cursor(ch);
            }
        }
    }

    /// Write a character at the current cursor position (delayed wrap).
    ///
    /// When the cursor is at the last column with `pending_wrap` set,
    /// the next printable character triggers the wrap first.
    fn write_char_at_cursor(&mut self, ch: char) {
        // If pending wrap, commit the wrap now before writing
        if self.pending_wrap {
            self.pending_wrap = false;
            self.cursor.0 = 0;
            if self.cursor.1 >= self.size.1.saturating_sub(1) {
                self.buffer.remove(0);
                self.buffer.push(vec![' '; self.size.0 as usize]);
            } else {
                self.cursor.1 += 1;
            }
        }

        if let Some(row) = self.buffer.get_mut(self.cursor.1 as usize)
            && let Some(cell) = row.get_mut(self.cursor.0 as usize)
        {
            *cell = ch;
            self.cursor.0 += 1;
            if self.cursor.0 >= self.size.0 {
                // Don't wrap immediately — set pending flag.
                // Cursor stays at last column visually.
                self.cursor.0 = self.size.0 - 1;
                self.pending_wrap = true;
            }
        }
    }

    /// Process a byte slice, handling ANSI escape sequences
    fn process_bytes(&mut self, buf: &[u8]) {
        let s = String::from_utf8_lossy(buf);
        let mut chars = s.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // Start of ANSI escape sequence
                if chars.peek() == Some(&'[') {
                    chars.next(); // consume '['
                    self.process_csi_sequence(&mut chars);
                } else if chars.peek() == Some(&'7') {
                    // DEC Save Cursor (ESC 7)
                    chars.next(); // consume '7'
                    self.saved_cursor = Some(self.cursor);
                } else if chars.peek() == Some(&'8') {
                    // DEC Restore Cursor (ESC 8)
                    chars.next(); // consume '8'
                    if let Some(saved) = self.saved_cursor {
                        self.cursor = saved;
                    }
                }
            } else {
                self.write_char(ch);
            }
        }
    }

    /// Process a CSI (Control Sequence Introducer) escape sequence
    #[allow(clippy::too_many_lines)]
    fn process_csi_sequence(&mut self, chars: &mut std::iter::Peekable<std::str::Chars>) {
        // Detect private mode prefix (e.g., `?` in `CSI ?2026h`)
        let private_mode = if chars.peek() == Some(&'?') {
            chars.next();
            true
        } else {
            false
        };

        let mut params = String::new();

        // Collect parameters (numbers and semicolons)
        while let Some(&ch) = chars.peek() {
            if ch.is_ascii_digit() || ch == ';' {
                params.push(ch);
                chars.next();
            } else {
                break;
            }
        }

        // Consume the final command character for private mode sequences and return
        if private_mode {
            chars.next(); // consume 'h', 'l', etc.
            return;
        }

        // Get the command character
        if let Some(cmd) = chars.next() {
            match cmd {
                'H' | 'f' => {
                    // Cursor Position (ANSI format is row;column, 1-indexed)
                    let parts: Vec<u16> =
                        params.split(';').filter_map(|s| s.parse().ok()).collect();
                    let row = parts.first().copied().unwrap_or(1).saturating_sub(1);
                    let col = parts.get(1).copied().unwrap_or(1).saturating_sub(1);
                    self.move_to(col, row); // move_to takes (col, row)
                }
                'A' => {
                    // Cursor Up
                    let n = params.parse().unwrap_or(1);
                    self.cursor.1 = self.cursor.1.saturating_sub(n);
                    self.pending_wrap = false;
                }
                'B' => {
                    // Cursor Down
                    let n = params.parse().unwrap_or(1);
                    self.cursor.1 = (self.cursor.1 + n).min(self.size.1.saturating_sub(1));
                    self.pending_wrap = false;
                }
                'C' => {
                    // Cursor Forward (Right)
                    let n = params.parse().unwrap_or(1);
                    self.move_right(n);
                }
                'D' => {
                    // Cursor Back (Left)
                    let n = params.parse().unwrap_or(1);
                    self.move_left(n);
                }
                'G' => {
                    // Cursor to Column
                    let col = params.parse::<u16>().unwrap_or(1).saturating_sub(1);
                    self.move_to_column(col);
                }
                'J' => {
                    // Erase in Display
                    let n = params.parse().unwrap_or(0);
                    match n {
                        0 => {
                            // Clear from cursor to end of screen
                            for row in self.cursor.1..self.size.1 {
                                if let Some(r) = self.buffer.get_mut(row as usize) {
                                    let start = if row == self.cursor.1 {
                                        self.cursor.0 as usize
                                    } else {
                                        0
                                    };
                                    for ch in r.iter_mut().skip(start) {
                                        *ch = ' ';
                                    }
                                }
                            }
                        }
                        2 => {
                            // Clear entire screen
                            self.clear();
                        }
                        _ => {}
                    }
                }
                'K' => {
                    // Erase in Line
                    let n = params.parse().unwrap_or(0);
                    match n {
                        0 => {
                            // Clear from cursor to end of line
                            if let Some(row) = self.buffer.get_mut(self.cursor.1 as usize) {
                                for ch in row.iter_mut().skip(self.cursor.0 as usize) {
                                    *ch = ' ';
                                }
                            }
                        }
                        2 => {
                            // Clear entire line
                            self.clear_line();
                        }
                        _ => {}
                    }
                }
                's' => {
                    // Save cursor position
                    self.saved_cursor = Some(self.cursor);
                }
                'u' => {
                    // Restore cursor position
                    if let Some(saved) = self.saved_cursor {
                        self.cursor = saved;
                        self.pending_wrap = false;
                    }
                }
                // SGR, unknown sequences — ignored (content-only testing)
                _ => {}
            }
        }
    }
}

impl Write for TestTerminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Accumulate all bytes in buffer
        self.escape_buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Process all accumulated bytes when flushed
        if !self.escape_buffer.is_empty() {
            let bytes = std::mem::take(&mut self.escape_buffer);
            self.process_bytes(&bytes);
        }
        Ok(())
    }
}

/// Asserts a test terminal buffer matches the expected output.
/// Each element of the expected vector represents a row.
/// Trailing whitespace is ignored on each line.
pub fn assert_buffer_eq<S: AsRef<str>>(terminal: &TestTerminal, expected: &[S]) {
    let actual_lines = terminal.get_lines();
    let max_lines = expected.len().max(actual_lines.len());

    for i in 0..max_lines {
        let expected_line = expected.get(i).map_or("", AsRef::as_ref);
        let actual_line = actual_lines.get(i).map_or("", String::as_str);

        assert_eq!(
            actual_line,
            expected_line,
            "Line {i} mismatch:\n  Expected: '{expected_line}'\n  Got:      '{actual_line}'\n\nFull buffer:\n{}",
            actual_lines.join("\n")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_write() {
        let mut term = TestTerminal::new(80, 24);
        write!(term, "Hello").unwrap();
        term.flush().unwrap();
        let lines = term.get_lines();
        assert_eq!(lines[0], "Hello");
    }

    #[test]
    fn test_newline() {
        let mut term = TestTerminal::new(80, 24);
        write!(term, "Line 1\nLine 2").unwrap();
        term.flush().unwrap();
        assert_buffer_eq(&term, &["Line 1", "Line 2"]);
    }

    #[test]
    fn test_carriage_return() {
        let mut term = TestTerminal::new(80, 24);
        write!(term, "Hello\rWorld").unwrap();
        term.flush().unwrap();
        let lines = term.get_lines();
        assert_eq!(lines[0], "World");
    }

    #[test]
    fn test_ansi_cursor_position() {
        let mut term = TestTerminal::new(80, 24);
        // CSI sequence for moving to row 3, column 5 (1-indexed)
        write!(term, "\x1b[3;5HX").unwrap();
        term.flush().unwrap();
        let lines = term.get_lines();
        assert_eq!(&lines[2][4..5], "X");
    }

    #[test]
    fn test_ansi_clear_line() {
        let mut term = TestTerminal::new(80, 24);
        write!(term, "Hello World").unwrap();
        // CSI K - clear from cursor to end of line
        write!(term, "\x1b[1G\x1b[K").unwrap();
        term.flush().unwrap();
        let lines = term.get_lines();
        assert_eq!(lines[0], "");
    }

    #[test]
    fn test_assert_buffer_eq() {
        let mut term = TestTerminal::new(80, 24);
        write!(term, "Line 1\nLine 2\nLine 3").unwrap();
        term.flush().unwrap();

        assert_buffer_eq(&term, &["Line 1", "Line 2", "Line 3"]);
    }

    #[test]
    #[should_panic(expected = "Line 0 mismatch")]
    fn test_assert_buffer_eq_fails() {
        let mut term = TestTerminal::new(80, 24);
        write!(term, "Wrong").unwrap();
        term.flush().unwrap();

        assert_buffer_eq(&term, &["Expected"]);
    }

    #[test]
    fn test_private_mode_sequences_ignored() {
        let mut term = TestTerminal::new(80, 24);
        // Synchronized update begin/end sequences should not leak into buffer
        write!(term, "\x1b[?2026hHello\x1b[?2026l").unwrap();
        term.flush().unwrap();
        let lines = term.get_lines();
        assert_eq!(lines[0], "Hello");
    }

    #[test]
    fn test_cursor_save_restore() {
        let mut term = TestTerminal::new(80, 24);

        // Move to (10, 5) and write "First" (cursor ends at col 15)
        write!(term, "\x1b[6;11HFirst").unwrap();

        // Save cursor position (should save col 15, row 5)
        write!(term, "\x1b7").unwrap(); // DEC save cursor

        // Move somewhere else and write
        write!(term, "\x1b[1;1HSecond").unwrap();

        // Restore cursor position (back to col 15, row 5) and write
        write!(term, "\x1b8Third").unwrap(); // DEC restore cursor

        term.flush().unwrap();

        let lines = term.get_lines();
        assert_eq!(lines[0], "Second");
        // "First" starts at column 10, cursor saved at 15, "Third" written at 15
        assert_eq!(lines[5], "          FirstThird");
    }
}
