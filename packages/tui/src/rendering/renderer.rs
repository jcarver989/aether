use super::frame::Frame;
use super::line::Line;
use super::render_context::ViewContext;
use super::terminal_screen::{TerminalCommand, TerminalScreen};
use super::visual_frame::VisualFrame;
use crate::rendering::render_context::Size;
use crate::theme::Theme;
use std::io::{self, Write};
use std::sync::Arc;

#[cfg(feature = "syntax")]
use crate::syntax_highlighting::SyntaxHighlighter;

pub enum RendererCommand {
    ClearScreen,
    SetTheme(Theme),
}

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
    prev_frame: Option<VisualFrame>,
    resized: bool,
}

impl<W: Write> Renderer<W> {
    pub fn new(writer: W, theme: Theme, size: impl Into<Size>) -> Self {
        Self {
            terminal: TerminalScreen::new(writer),
            size: size.into(),
            theme: Arc::new(theme),
            #[cfg(feature = "syntax")]
            highlighter: Arc::new(SyntaxHighlighter::new()),
            prev_frame: None,
            resized: false,
        }
    }

    /// Render a frame using a closure.
    ///
    /// The closure receives a `ViewContext` and returns a Frame.
    pub fn render_frame(&mut self, f: impl FnOnce(&ViewContext) -> Frame) -> io::Result<()> {
        let context = self.context();
        let frame = f(&context).clamp_cursor();
        self.render_frame_internal(&frame)
    }

    pub fn clear_screen(&mut self) -> io::Result<()> {
        let commands = vec![TerminalCommand::ClearAll];
        self.terminal.execute(&commands)?;
        self.prev_frame = None;
        self.resized = false;
        Ok(())
    }

    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()> {
        self.push_lines_to_scrollback(lines, self.size.width)
    }

    pub fn on_resize(&mut self, size: impl Into<Size>) {
        self.size = size.into();
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
        self.theme = Arc::new(theme);
    }

    pub fn apply_commands(&mut self, commands: Vec<RendererCommand>) -> io::Result<()> {
        for cmd in commands {
            match cmd {
                RendererCommand::ClearScreen => self.clear_screen()?,
                RendererCommand::SetTheme(theme) => self.set_theme(theme),
            }
        }
        Ok(())
    }

    pub fn writer(&self) -> &W {
        &self.terminal.writer
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn test_writer_mut(&mut self) -> &mut W {
        &mut self.terminal.writer
    }

    #[cfg(test)]
    fn flushed_visual_count(&self) -> usize {
        self.prev_frame.as_ref().map_or(0, |f| f.overflow())
    }

    fn render_frame_internal(&mut self, frame: &Frame) -> io::Result<()> {
        let next_frame = {
            let flushed = self.prev_frame.as_ref().map_or(0, |f| f.overflow());
            VisualFrame::from_frame(frame, self.size, flushed)
        };

        // When resized, the previous frame layout is invalid — start fresh.
        let (mut commands, mut prev_frame) = if self.resized {
            (vec![TerminalCommand::ClearViewport], None)
        } else {
            (
                vec![TerminalCommand::RestoreCursorPosition],
                self.prev_frame.as_ref(),
            )
        };

        if !next_frame.scrollback_lines().is_empty() {
            commands.push(TerminalCommand::PushScrollbackLines {
                previous_visible_rows: prev_frame.map_or(0, |f| f.visible_lines().len()),
                lines: next_frame.scrollback_lines(),
            });
            prev_frame = None;
        }

        let empty = VisualFrame::empty();
        if let Some(diff) = prev_frame.unwrap_or(&empty).diff(&next_frame) {
            commands.push(Self::rewrite_command(&diff));
        }

        commands.extend(Self::cursor_commands(&next_frame));
        self.terminal.execute(&commands)?;
        self.prev_frame = Some(next_frame);
        self.resized = false;
        Ok(())
    }

    fn rewrite_command<'a>(diff: &super::visual_frame::LineDiff<'a>) -> TerminalCommand<'a> {
        TerminalCommand::RewriteVisibleLines {
            rows_up: diff_rows_up(diff),
            append_after_existing: diff_should_append(diff),
            lines: diff.lines,
        }
    }

    fn cursor_commands(frame: &VisualFrame) -> [TerminalCommand<'_>; 2] {
        let cursor = frame.cursor();
        let rows_up = to_u16(
            frame
                .visible_lines()
                .len()
                .saturating_sub(1)
                .saturating_sub(cursor.row),
        );

        [
            TerminalCommand::SetCursorVisible(cursor.is_visible),
            TerminalCommand::PlaceCursor {
                rows_up,
                col: to_u16(cursor.col),
            },
        ]
    }

    fn push_lines_to_scrollback(&mut self, lines: &[Line], width: u16) -> io::Result<()> {
        use super::visual_frame::prepare_lines_for_scrollback;

        let visual = prepare_lines_for_scrollback(lines, width);

        if visual.is_empty() {
            self.prev_frame = None;
            return Ok(());
        }

        let flushed = self.prev_frame.as_ref().map_or(0, |f| f.overflow());
        let remaining = &visual[flushed.min(visual.len())..];
        let mut commands = vec![TerminalCommand::RestoreCursorPosition];

        if remaining.is_empty() {
            self.prev_frame = None;
            self.terminal.execute(&commands)?;
            return Ok(());
        }

        let previous_visible_rows = self
            .prev_frame
            .as_ref()
            .map_or(0, |f| f.visible_lines().len());
        commands.push(TerminalCommand::PushScrollbackLines {
            previous_visible_rows,
            lines: remaining,
        });

        self.terminal.execute(&commands)?;
        self.prev_frame = None;
        Ok(())
    }
}

fn diff_rows_up(diff: &super::visual_frame::LineDiff<'_>) -> u16 {
    if diff.rewrite_from < diff.previous_row_count {
        to_u16(diff.previous_row_count - 1 - diff.rewrite_from)
    } else {
        0
    }
}

fn diff_should_append(diff: &super::visual_frame::LineDiff<'_>) -> bool {
    diff.rewrite_from >= diff.previous_row_count && diff.previous_row_count > 0
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
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
        Frame::new(lines.iter().map(|line| Line::new(*line)).collect()).with_cursor(Cursor {
            row: lines.len().saturating_sub(1),
            col: 0,
            is_visible: true,
        })
    }

    #[test]
    fn set_theme_replaces_render_context_theme() {
        let mut renderer = Renderer::new(Vec::new(), Theme::default(), (80, 24));
        let new_theme = Theme::default();
        let expected = new_theme.text_primary();
        renderer.set_theme(new_theme);
        assert_eq!(renderer.context().theme.text_primary(), expected);
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn set_theme_replaces_render_context_theme_from_file() {
        let mut renderer = Renderer::new(Vec::new(), Theme::default(), (80, 24));
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
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 24));
        let empty_frame = Frame::new(vec![]);
        renderer.render_frame_internal(&empty_frame).unwrap();
        renderer.terminal.writer.bytes.clear();
        renderer.render_frame_internal(&empty_frame).unwrap();
        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        assert!(
            !output.contains("\x1b[J"),
            "should not clear from cursor down on identical empty frames"
        );
    }

    #[test]
    fn first_render_writes_all_lines() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 24));
        let f = frame(&["hello", "world"]);
        renderer.render_frame_internal(&f).unwrap();
        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        assert!(output.contains("hello"));
        assert!(output.contains("world"));
    }

    #[test]
    fn identical_frames_produce_no_visible_rewrites() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 24));
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
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 24));
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
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 24));
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
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 2));
        let frame = Frame::new(vec![
            Line::new("L1"),
            Line::new("L2"),
            Line::new("L3"),
            Line::new("L4"),
        ])
        .with_cursor(Cursor {
            row: 2,
            col: 0,
            is_visible: true,
        });
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
    fn push_to_scrollback_clears_prev_visible_lines() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 24));

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
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 24));
        renderer.push_to_scrollback(&[]).unwrap();
        assert!(renderer.terminal.writer.bytes.is_empty());
    }

    #[test]
    fn clear_screen_emits_clear_all_and_purge() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 24));
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
    fn clear_screen_resets_resize_state() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 24));
        renderer.clear_screen().unwrap();

        renderer.terminal.writer.bytes.clear();
        renderer.render_frame_internal(&frame(&["hello"])).unwrap();

        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        assert!(output.contains("hello"));
        assert!(
            !output.contains("\x1b[2J"),
            "render after clear_screen should not still clear viewport, got: {:?}",
            output
        );
    }

    #[test]
    fn resize_marks_terminal_for_full_clear_and_redraw() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (10, 4));
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
    fn visual_frame_splits_overflow_from_visible_lines() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (80, 2));

        let f = Frame::new(vec![
            Line::new("L1"),
            Line::new("L2"),
            Line::new("L3"),
            Line::new("L4"),
        ])
        .with_cursor(Cursor {
            row: 3,
            col: 0,
            is_visible: true,
        });
        renderer.render_frame_internal(&f).unwrap();

        assert_eq!(renderer.flushed_visual_count(), 2);
    }

    #[test]
    fn cursor_remapped_after_wrap() {
        let mut renderer = Renderer::new(FakeWriter::new(), Theme::default(), (3, 24));

        let f = Frame::new(vec![Line::new("abcdef")]).with_cursor(Cursor {
            row: 0,
            col: 5,
            is_visible: true,
        });
        renderer.render_frame_internal(&f).unwrap();

        let output = String::from_utf8_lossy(&renderer.terminal.writer.bytes);
        assert!(
            output.contains("\x1b[2C"),
            "cursor should be at col 2 (MoveRight(2)), got: {:?}",
            output
        );
    }
}
