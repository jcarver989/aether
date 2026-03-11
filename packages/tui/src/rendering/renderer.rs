use std::io::{self, Write};
use std::sync::Arc;

use super::frame::Frame;
use super::line::Line;
use super::prepared_frame::PreparedFrame;
use super::render_context::ViewContext;
use super::size::Size;
use super::terminal_screen::{TerminalCommand, TerminalScreen};
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
    terminal: TerminalScreen<W>,
    size: Size,
    theme: Arc<Theme>,
    #[cfg(feature = "syntax")]
    highlighter: Arc<SyntaxHighlighter>,
    render_epoch: u64,
    prev_frame: Vec<Line>,
    last_width: u16,
    flushed_visual_count: usize,
    resized: bool,
}

impl<W: Write> Renderer<W> {
    pub fn new(writer: W, theme: Theme) -> Self {
        Self {
            terminal: TerminalScreen::new(writer),
            size: (0, 0).into(),
            theme: Arc::new(theme),
            #[cfg(feature = "syntax")]
            highlighter: Arc::new(SyntaxHighlighter::new()),
            render_epoch: 0,
            prev_frame: Vec::new(),
            last_width: 0,
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
        self.terminal.execute(&[TerminalCommand::ClearAll])?;
        self.prev_frame.clear();
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
        self.terminal.reset_cursor_offset();
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
        &self.terminal.writer
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn test_writer_mut(&mut self) -> &mut W {
        &mut self.terminal.writer
    }

    pub fn render_epoch(&self) -> u64 {
        self.render_epoch
    }

    #[cfg(test)]
    fn flushed_visual_count(&self) -> usize {
        self.flushed_visual_count
    }

    fn bump_render_epoch(&mut self) {
        self.render_epoch = self.render_epoch.wrapping_add(1);
    }

    fn render_frame_internal(&mut self, frame: &Frame) -> io::Result<()> {
        let prepared = PreparedFrame::new(
            frame.lines(),
            frame.cursor(),
            self.size,
            self.flushed_visual_count,
        );

        if self.resized {
            self.terminal.execute(&[TerminalCommand::ClearViewport])?;
            self.prev_frame.clear();
            if !prepared.scrollback_lines().is_empty() {
                self.push_visual_to_scrollback(prepared.scrollback_lines())?;
            }
            self.flushed_visual_count = prepared.overflow();
            self.resized = false;
        } else {
            self.terminal
                .execute(&[TerminalCommand::RestoreCursorPosition])?;
            if !prepared.scrollback_lines().is_empty() {
                self.push_visual_to_scrollback(prepared.scrollback_lines())?;
                self.flushed_visual_count = prepared.overflow();
            }
        }

        self.render_visible(prepared.visible_lines(), self.size.width)?;
        let rows_up = u16::try_from(
            prepared
                .visible_lines()
                .len()
                .saturating_sub(1)
                .saturating_sub(prepared.cursor().row),
        )
        .unwrap_or(u16::MAX);

        self.terminal.execute(&[
            TerminalCommand::SetCursorVisible(prepared.cursor().is_visible),
            TerminalCommand::PlaceCursor {
                rows_up,
                col: u16::try_from(prepared.cursor().col).unwrap_or(u16::MAX),
            },
        ])?;
        Ok(())
    }

    fn push_lines_to_scrollback(&mut self, lines: &[Line], width: u16) -> io::Result<()> {
        use super::soft_wrap::soft_wrap_line;

        let visual: Vec<Line> = lines
            .iter()
            .flat_map(|line| soft_wrap_line(line, width))
            .collect();

        if visual.is_empty() {
            self.flushed_visual_count = 0;
            return Ok(());
        }

        let remaining = &visual[self.flushed_visual_count.min(visual.len())..];

        self.terminal
            .execute(&[TerminalCommand::RestoreCursorPosition])?;

        if remaining.is_empty() {
            self.flushed_visual_count = 0;
            return Ok(());
        }

        self.push_visual_to_scrollback(remaining)?;
        self.flushed_visual_count = 0;
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

        let to_write = &new_frame[rewrite_from..];
        let append_after_existing = rewrite_from >= prev_on_screen && prev_on_screen > 0;

        let rows_up = if rewrite_from < prev_on_screen {
            u16::try_from(prev_on_screen - 1 - rewrite_from).unwrap_or(u16::MAX)
        } else {
            0
        };

        self.terminal
            .execute(&[TerminalCommand::RewriteVisibleLines {
                rows_up,
                append_after_existing,
                lines: to_write,
            }])?;

        let lines_written = to_write.len();
        self.prev_frame = new_frame.to_vec();
        Ok(lines_written)
    }

    fn push_visual_to_scrollback(&mut self, visual_lines: &[Line]) -> io::Result<()> {
        if visual_lines.is_empty() {
            return Ok(());
        }

        let prev_frame_len = self.prev_frame.len();

        self.terminal
            .execute(&[TerminalCommand::PushScrollbackLines {
                previous_visible_rows: prev_frame_len,
                lines: visual_lines,
            }])?;

        self.prev_frame.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::frame::Cursor;

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
        assert!(renderer.terminal.writer.bytes.is_empty());
    }

    #[test]
    fn first_render_writes_all_lines() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        let f = frame(&["hello", "world"]);
        renderer.render_frame_internal(&f).unwrap();
        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        assert!(output.contains("hello"));
        assert!(output.contains("world"));
    }

    #[test]
    fn identical_frames_produce_no_visible_rewrites() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        let f = frame(&["hello", "world"]);
        renderer.render_frame_internal(&f).unwrap();

        renderer.terminal.writer.bytes.clear();
        renderer.render_frame_internal(&f).unwrap();
        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        assert!(!output.contains("hello"));
        assert!(!output.contains("world"));
    }

    #[test]
    fn changing_middle_line_rewrites_from_diff() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        let frame1 = frame(&["aaa", "bbb", "ccc"]);
        renderer.render_frame_internal(&frame1).unwrap();

        renderer.terminal.writer.bytes.clear();
        let frame2 = frame(&["aaa", "BBB", "ccc"]);
        renderer.render_frame_internal(&frame2).unwrap();
        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        assert!(output.contains("BBB"));
        assert!(output.contains("ccc"));
    }

    #[test]
    fn appending_line_moves_to_next_row_before_writing() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        let frame1 = frame(&["aaa", "bbb"]);
        renderer.render_frame_internal(&frame1).unwrap();

        renderer.terminal.writer.bytes.clear();
        let frame2 = frame(&["aaa", "bbb", "ccc"]);
        renderer.render_frame_internal(&frame2).unwrap();

        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        let ccc_index = output.find("ccc").expect("missing appended line");
        assert!(
            output[..ccc_index].contains("\r\n"),
            "append rewrite should move to the next row before writing, got: {:?}",
            output
        );
    }

    #[test]
    fn push_to_scrollback_restores_cursor_even_when_nothing_new_is_flushed() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 2));
        let frame = Frame::new(
            vec![
                Line::new("L1"),
                Line::new("L2"),
                Line::new("L3"),
                Line::new("L4"),
            ],
            Cursor {
                row: 2,
                col: 0,
                is_visible: true,
            },
        );
        renderer.render_frame_internal(&frame).unwrap();
        renderer.terminal.writer.bytes.clear();

        renderer
            .push_to_scrollback(&[
                Line::new("already flushed 1"),
                Line::new("already flushed 2"),
            ])
            .unwrap();

        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        assert!(
            output.contains("\x1b[1B"),
            "push_to_scrollback should restore cursor before early return, got: {:?}",
            output
        );
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

        renderer.terminal.writer.bytes.clear();
        renderer.render_frame_internal(&f).unwrap();
        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        assert!(output.contains("managed line"));
    }

    #[test]
    fn push_to_scrollback_empty_is_noop() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.on_resize((80, 24));
        renderer.push_to_scrollback(&[]).unwrap();
        assert!(renderer.terminal.writer.bytes.is_empty());
    }

    #[test]
    fn clear_screen_emits_clear_all_and_purge() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default());
        renderer.clear_screen().unwrap();
        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
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

        renderer.terminal.writer.bytes.clear();
        renderer.on_resize((5, 4));
        let narrow = frame(&["abcdefghij"]);
        renderer.render_frame_internal(&narrow).unwrap();

        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
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

        assert_eq!(renderer.flushed_visual_count(), 2);
    }
}
