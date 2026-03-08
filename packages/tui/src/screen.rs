use crossterm::{
    QueueableCommand,
    cursor::MoveUp,
    style::{Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate},
};
use std::fmt::Write as _;
use std::io::{self, Write};
use unicode_width::UnicodeWidthStr;

/// A single line of pre-formatted terminal output.
/// Holds text and style spans. ANSI is emitted only at write-time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    spans: Vec<Span>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub strikethrough: bool,
}

impl Style {
    pub fn fg(color: Color) -> Self {
        Self::default().color(color)
    }

    pub fn color(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    pub fn bg_color(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    #[allow(dead_code)]
    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// Merge `other` on top of `self`. Booleans are OR'd, `Option` fields
    /// prefer `other` when `Some`.
    pub fn merge(self, other: Self) -> Self {
        Self {
            fg: other.fg.or(self.fg),
            bg: other.bg.or(self.bg),
            bold: self.bold || other.bold,
            italic: self.italic || other.italic,
            underline: self.underline || other.underline,
            dim: self.dim || other.dim,
            strikethrough: self.strikethrough || other.strikethrough,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    text: String,
    style: Style,
}

impl Span {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
        }
    }

    pub fn with_style(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn style(&self) -> Style {
        self.style
    }
}

impl Line {
    pub fn new(s: impl Into<String>) -> Self {
        let text = s.into();
        if text.is_empty() {
            return Self::default();
        }

        Self {
            spans: vec![Span::new(text)],
        }
    }

    pub fn styled(text: impl Into<String>, color: Color) -> Self {
        Self::with_style(text, Style::fg(color))
    }

    pub fn with_style(text: impl Into<String>, style: Style) -> Self {
        let text = text.into();
        if text.is_empty() {
            return Self::default();
        }

        Self {
            spans: vec![Span::with_style(text, style)],
        }
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    pub fn push_text(&mut self, text: impl Into<String>) {
        self.push_span(Span::new(text));
    }

    pub fn push_styled(&mut self, text: impl Into<String>, color: Color) {
        self.push_with_style(text, Style::fg(color));
    }

    pub fn push_with_style(&mut self, text: impl Into<String>, style: Style) {
        self.push_span(Span::with_style(text, style));
    }

    pub fn push_span(&mut self, span: Span) {
        if span.text.is_empty() {
            return;
        }

        if let Some(last) = self.spans.last_mut()
            && last.style == span.style
        {
            last.text.push_str(&span.text);
            return;
        }
        self.spans.push(span);
    }

    pub fn append_line(&mut self, other: &Line) {
        for span in &other.spans {
            self.push_span(span.clone());
        }
    }

    pub fn extend_bg_to_width(&mut self, target_width: usize) {
        let current_width = UnicodeWidthStr::width(self.plain_text().as_str());
        let pad = target_width.saturating_sub(current_width);
        if pad == 0 {
            return;
        }

        let bg = self.spans.iter().find_map(|span| span.style().bg);
        if let Some(bg) = bg {
            self.push_with_style(
                format!("{:pad$}", "", pad = pad),
                Style::default().bg_color(bg),
            );
        } else {
            self.push_text(format!("{:pad$}", "", pad = pad));
        }
    }

    pub fn to_ansi_string(&self) -> String {
        if self.spans.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        let mut active_style = Style::default();

        for span in &self.spans {
            if span.style != active_style {
                emit_style_transition(&mut out, active_style, span.style);
                active_style = span.style;
            }
            out.push_str(&span.text);
        }

        if active_style != Style::default() {
            emit_style_transition(&mut out, active_style, Style::default());
        }

        out
    }

    #[allow(dead_code)]
    pub fn plain_text(&self) -> String {
        let mut text = String::new();
        for span in &self.spans {
            text.push_str(&span.text);
        }
        text
    }
}

impl std::fmt::Display for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for span in &self.spans {
            f.write_str(&span.text)?;
        }
        Ok(())
    }
}

fn push_fg_sgr(out: &mut String, color: Option<Color>) {
    let fg = color.unwrap_or(Color::Reset);
    let _ = write!(out, "{}", SetForegroundColor(fg));
}

fn push_bg_sgr(out: &mut String, color: Option<Color>) {
    let bg = color.unwrap_or(Color::Reset);
    let _ = write!(out, "{}", SetBackgroundColor(bg));
}

fn push_attr_sgr(out: &mut String, attr: Attribute) {
    let _ = write!(out, "{}", SetAttribute(attr));
}

fn emit_style_transition(out: &mut String, from: Style, to: Style) {
    // Check if any boolean attribute turned OFF — requires a full reset
    let needs_reset = (from.bold && !to.bold)
        || (from.italic && !to.italic)
        || (from.underline && !to.underline)
        || (from.dim && !to.dim)
        || (from.strikethrough && !to.strikethrough);

    if needs_reset {
        push_attr_sgr(out, Attribute::Reset);
        // After reset, re-emit all active attributes and colors on `to`
        if to.bold {
            push_attr_sgr(out, Attribute::Bold);
        }
        if to.italic {
            push_attr_sgr(out, Attribute::Italic);
        }
        if to.underline {
            push_attr_sgr(out, Attribute::Underlined);
        }
        if to.dim {
            push_attr_sgr(out, Attribute::Dim);
        }
        if to.strikethrough {
            push_attr_sgr(out, Attribute::CrossedOut);
        }
        push_fg_sgr(out, to.fg);
        push_bg_sgr(out, to.bg);
        return;
    }

    // Only turning attributes ON — emit incrementally
    if !from.bold && to.bold {
        push_attr_sgr(out, Attribute::Bold);
    }
    if !from.italic && to.italic {
        push_attr_sgr(out, Attribute::Italic);
    }
    if !from.underline && to.underline {
        push_attr_sgr(out, Attribute::Underlined);
    }
    if !from.dim && to.dim {
        push_attr_sgr(out, Attribute::Dim);
    }
    if !from.strikethrough && to.strikethrough {
        push_attr_sgr(out, Attribute::CrossedOut);
    }
    if from.fg != to.fg {
        push_fg_sgr(out, to.fg);
    }
    if from.bg != to.bg {
        push_bg_sgr(out, to.bg);
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
    last_width: u16,
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

    #[test]
    fn builder_style_supports_bold_and_color() {
        let mut line = Line::default();
        line.push_with_style("hot", Style::default().bold().color(Color::Red));

        let ansi = line.to_ansi_string();
        let mut bold = String::new();
        let mut red = String::new();
        let mut reset_attr = String::new();
        push_attr_sgr(&mut bold, Attribute::Bold);
        push_fg_sgr(&mut red, Some(Color::Red));
        push_attr_sgr(&mut reset_attr, Attribute::Reset);

        assert!(ansi.contains(&bold));
        assert!(ansi.contains(&red));
        assert!(ansi.contains("hot"));
        // When bold turns off, a full Reset is emitted
        assert!(ansi.contains(&reset_attr));
    }
}
