use crossterm::{
    QueueableCommand,
    cursor::{MoveTo, MoveUp},
    terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate},
};
use std::io::{self, Write};
use std::sync::Arc;

use super::frame::{Cursor, Frame};
use super::line::Line;
use super::render_context::ViewContext;
use super::size::Size;
use crate::theme::Theme;

#[cfg(feature = "syntax")]
use crate::syntax_highlighting::SyntaxHighlighter;

/// Pure TUI renderer with frame diffing and terminal state management.
///
/// Uses relative cursor movement (`MoveUp` + `\r`) to navigate back to the
/// start of the managed region. This avoids absolute row tracking, which breaks
/// when the terminal scrolls content upward.
///
/// **Cursor invariant:** After every render or `push_to_scrollback`, the
/// cursor sits at the end of the last managed line unless explicitly
/// repositioned afterward.
pub struct Renderer<W: Write> {
    writer: W,
    size: Size,
    theme: Arc<Theme>,
    #[cfg(feature = "syntax")]
    highlighter: Arc<SyntaxHighlighter>,
    render_epoch: u64,
    prev_frame: Vec<Line>,
    last_width: u16,
    cursor_row_offset: u16,
    cursor_visible: bool,
    flushed_visual_count: usize,
    resized: bool,
}

impl<W: Write> Renderer<W> {
    pub fn new(writer: W, theme: Theme) -> Self {
        Self {
            writer,
            size: (0, 0).into(),
            theme: Arc::new(theme),
            #[cfg(feature = "syntax")]
            highlighter: Arc::new(SyntaxHighlighter::new()),
            render_epoch: 0,
            prev_frame: Vec::new(),
            last_width: 0,
            cursor_row_offset: 0,
            cursor_visible: true,
            flushed_visual_count: 0,
            resized: false,
        }
    }

    /// Render a frame using a closure.
    ///
    /// The closure receives a ViewContext and returns a Frame.
    pub fn render_frame(&mut self, f: impl FnOnce(&ViewContext) -> Frame) -> io::Result<()> {
        let context = self.context();
        let frame = f(&context).soft_wrap(self.size.width).clamp_cursor();
        self.render_frame_internal(&frame)
    }

    pub fn clear_screen(&mut self) -> io::Result<()> {
        self.bump_render_epoch();
        self.writer.queue(Clear(ClearType::All))?;
        self.writer.queue(Clear(ClearType::Purge))?;
        self.writer.queue(MoveTo(0, 0))?;
        self.writer.flush()?;
        self.prev_frame.clear();
        self.cursor_row_offset = 0;
        self.flushed_visual_count = 0;
        Ok(())
    }

    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()> {
        self.bump_render_epoch();
        self.push_lines_to_scrollback(lines, self.size.width)
    }

    pub fn on_resize(&mut self, size: impl Into<Size>) {
        self.bump_render_epoch();
        self.size = size.into();
        self.last_width = self.size.width;
        self.prev_frame.clear();
        self.cursor_row_offset = 0;
        self.resized = true;
    }

    pub fn context(&self) -> ViewContext {
        ViewContext {
            size: self.size,
            theme: self.theme.clone(),
            #[cfg(feature = "syntax")]
            highlighter: self.highlighter.clone(),
        }
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.bump_render_epoch();
        self.theme = Arc::new(theme);
    }

    pub fn writer(&self) -> &W {
        &self.writer
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn test_writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    pub fn render_epoch(&self) -> u64 {
        self.render_epoch
    }

    fn bump_render_epoch(&mut self) {
        self.render_epoch = self.render_epoch.wrapping_add(1);
    }

    fn render_frame_internal(&mut self, frame: &Frame) -> io::Result<()> {
        let lines = frame.lines();
        let cursor = frame.cursor();
        let viewport_rows = usize::from(self.size.height.max(1));
        let overflow = lines.len().saturating_sub(viewport_rows);

        let scrollback_lines = if overflow > self.flushed_visual_count {
            lines[self.flushed_visual_count..overflow].to_vec()
        } else {
            Vec::new()
        };

        let visible_lines = lines[overflow..].to_vec();
        let mut cursor_row = cursor.row.min(lines.len().saturating_sub(1));
        cursor_row = cursor_row.saturating_sub(overflow);
        if cursor_row >= visible_lines.len() {
            cursor_row = visible_lines.len().saturating_sub(1);
        }

        let adjusted_cursor = Cursor {
            row: cursor_row,
            col: cursor.col,
            is_visible: cursor.is_visible,
        };

        if self.resized {
            self.clear_viewport()?;
            if !scrollback_lines.is_empty() {
                self.push_visual_to_scrollback(&scrollback_lines)?;
            }
            self.flushed_visual_count = overflow;
            self.resized = false;
        } else {
            self.restore_cursor_position()?;

            if !scrollback_lines.is_empty() {
                self.push_visual_to_scrollback(&scrollback_lines)?;
                self.flushed_visual_count = overflow;
            }
        }

        self.render_visible(&visible_lines, self.size.width)?;
        self.set_cursor_visible(adjusted_cursor.is_visible)?;

        let rows_up = u16::try_from(
            visible_lines
                .len()
                .saturating_sub(1)
                .saturating_sub(adjusted_cursor.row),
        )
        .unwrap_or(u16::MAX);
        self.reposition_cursor(
            rows_up,
            u16::try_from(adjusted_cursor.col).unwrap_or(u16::MAX),
        )?;
        Ok(())
    }

    fn push_lines_to_scrollback(&mut self, lines: &[Line], width: u16) -> io::Result<()> {
        use super::soft_wrap::soft_wrap_line;

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

    fn clear_viewport(&mut self) -> io::Result<()> {
        self.writer.queue(BeginSynchronizedUpdate)?;
        self.writer.queue(Clear(ClearType::All))?;
        self.writer.queue(MoveTo(0, 0))?;
        self.writer.queue(EndSynchronizedUpdate)?;
        self.writer.flush()?;
        self.prev_frame.clear();
        self.cursor_row_offset = 0;
        Ok(())
    }

    fn render_visible(&mut self, new_frame: &[Line], width: u16) -> io::Result<usize> {
        let prev_on_screen = self.prev_frame.len();

        if width != self.last_width {
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

    fn frame(lines: &[&str]) -> Frame {
        Frame::new(
            lines.iter().map(|line| Line::new(*line)).collect(),
            Cursor {
                row: lines.len().saturating_sub(1),
                col: 0,
                is_visible: true,
            },
        )
    }

    #[test]
    fn set_theme_replaces_render_context_theme() {
        let mut renderer = Renderer::new(Vec::new(), Theme::default());
        let new_theme = Theme::default();
        let expected = new_theme.text_primary();

        renderer.set_theme(new_theme);

        assert_eq!(renderer.context().theme.text_primary(), expected);
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn set_theme_replaces_render_context_theme_from_file() {
        let mut renderer = Renderer::new(Vec::new(), Theme::default());

        let custom_tmtheme = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>name</key>
    <string>Custom</string>
    <key>settings</key>
    <array>
        <dict>
            <key>settings</key>
            <dict>
                <key>foreground</key>
                <string>#112233</string>
                <key>background</key>
                <string>#000000</string>
            </dict>
        </dict>
    </array>
</dict>
</plist>"#;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let theme_path = temp_dir.path().join("custom.tmTheme");
        std::fs::write(&theme_path, custom_tmtheme).unwrap();

        let loaded = Theme::load_from_path(&theme_path);
        renderer.set_theme(loaded);

        assert_eq!(
            renderer.context().theme.text_primary(),
            crossterm::style::Color::Rgb {
                r: 0x11,
                g: 0x22,
                b: 0x33
            }
        );
    }

    #[test]
    fn empty_to_empty_is_noop() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        renderer.render_visible(&[], 80).unwrap();
        assert!(renderer.writer.bytes.is_empty());
    }

    #[test]
    fn first_render_writes_all_lines() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        let f = frame(&["hello", "world"]);
        renderer.render_frame_internal(&f).unwrap();
        let output = String::from_utf8_lossy(&renderer.writer.bytes);
        assert!(output.contains("hello"));
        assert!(output.contains("world"));
    }

    #[test]
    fn identical_frames_produce_no_visible_rewrites() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        let f = frame(&["hello", "world"]);
        renderer.render_frame_internal(&f).unwrap();

        renderer.writer.bytes.clear();
        renderer.render_frame_internal(&f).unwrap();
        let output = String::from_utf8_lossy(&renderer.writer.bytes);
        assert!(!output.contains("hello"));
        assert!(!output.contains("world"));
    }

    #[test]
    fn changing_middle_line_rewrites_from_diff() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        let frame1 = frame(&["aaa", "bbb", "ccc"]);
        renderer.render_frame_internal(&frame1).unwrap();

        renderer.writer.bytes.clear();
        let frame2 = frame(&["aaa", "BBB", "ccc"]);
        renderer.render_frame_internal(&frame2).unwrap();
        let output = String::from_utf8_lossy(&renderer.writer.bytes);
        assert!(output.contains("BBB"));
        assert!(output.contains("ccc"));
    }

    #[test]
    fn push_to_scrollback_clears_prev_frame() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));

        let f = frame(&["managed line"]);
        renderer.render_frame_internal(&f).unwrap();

        renderer
            .push_to_scrollback(&[Line::new("scrolled")])
            .unwrap();

        renderer.writer.bytes.clear();
        renderer.render_frame_internal(&f).unwrap();
        let output = String::from_utf8_lossy(&renderer.writer.bytes);
        assert!(output.contains("managed line"));
    }

    #[test]
    fn push_to_scrollback_empty_is_noop() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        renderer.push_to_scrollback(&[]).unwrap();
        assert!(renderer.writer.bytes.is_empty());
    }

    #[test]
    fn clear_screen_emits_clear_all_and_purge() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.clear_screen().unwrap();
        let output = String::from_utf8_lossy(&renderer.writer.bytes);
        assert!(output.contains("\x1b[2J"), "missing ClearType::All");
        assert!(output.contains("\x1b[3J"), "missing ClearType::Purge");
        assert!(
            output.contains("\x1b[1;1H"),
            "missing cursor home (MoveTo(0,0))"
        );
    }

    #[test]
    fn resize_marks_terminal_for_full_clear_and_redraw() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((10, 4));
        let wide = frame(&["abcdefghij"]);
        renderer.render_frame_internal(&wide).unwrap();

        renderer.writer.bytes.clear();
        renderer.on_resize((5, 4));
        let narrow = frame(&["abcdefghij"]);
        renderer.render_frame_internal(&narrow).unwrap();

        let output = String::from_utf8_lossy(&renderer.writer.bytes);
        assert!(output.contains("\x1b[2J") || output.contains("\x1b[H"));
        assert!(output.contains("abcde"));
        assert!(output.contains("fghij"));
    }

    #[test]
    fn prepared_frame_splits_overflow_from_visible_lines() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 2));

        let f = Frame::new(
            vec![
                Line::new("L1"),
                Line::new("L2"),
                Line::new("L3"),
                Line::new("L4"),
            ],
            Cursor {
                row: 3,
                col: 0,
                is_visible: true,
            },
        );
        renderer.render_frame_internal(&f).unwrap();

        assert_eq!(renderer.flushed_visual_count, 2);
    }
}
