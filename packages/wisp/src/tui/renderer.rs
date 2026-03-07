use super::component::RenderContext;
use super::screen::{Line, Screen};
use super::soft_wrap::{soft_wrap_line, soft_wrap_lines_with_map};
use super::theme::Theme;
use crossterm::QueueableCommand;
use crossterm::cursor::{Hide, MoveDown, Show};
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub logical_row: usize,
    pub col: usize,
}

pub struct RenderOutput {
    pub lines: Vec<Line>,
    pub cursor: Cursor,
    pub cursor_visible: bool,
}

pub trait CursorComponent {
    fn render_with_cursor(&mut self, context: &RenderContext) -> RenderOutput;
}

/// Pure TUI renderer that owns terminal output, frame diffing, and cursor state.
pub struct Renderer<T: Write> {
    writer: T,
    screen: Screen,
    context: RenderContext,
    /// How many rows above the last frame line the cursor currently sits.
    /// 0 = cursor at last line (Screen's default assumption).
    cursor_row_offset: u16,
    cursor_visible: bool,
    /// How many visual lines have already been flushed to scrollback
    /// via progressive overflow handling.
    flushed_visual_count: usize,
}

impl<T: Write> Renderer<T> {
    pub fn new(writer: T, theme: Theme) -> Self {
        let context = RenderContext::new((0, 0)).with_theme(theme);
        Self {
            writer,
            screen: Screen::new(),
            context,
            cursor_row_offset: 0,
            cursor_visible: true,
            flushed_visual_count: 0,
        }
    }

    pub fn render<C: CursorComponent + ?Sized>(&mut self, root: &mut C) -> io::Result<()> {
        let output = root.render_with_cursor(&self.context);
        let (full_visual_lines, logical_to_visual) =
            soft_wrap_lines_with_map(&output.lines, self.context.size.0);

        let mut cursor_row = logical_to_visual
            .get(output.cursor.logical_row)
            .copied()
            .unwrap_or_else(|| full_visual_lines.len().saturating_sub(1));

        let width = usize::from(self.context.size.0);
        let mut cursor_col = output.cursor.col;
        if width > 0 {
            cursor_row += cursor_col / width;
            cursor_col %= width;
        } else {
            cursor_col = 0;
        }

        if cursor_row >= full_visual_lines.len() {
            cursor_row = full_visual_lines.len().saturating_sub(1);
        }

        // Progressively flush overflow lines to terminal scrollback so the
        // user can scroll up to see the full response.
        let viewport_rows = usize::from(self.context.size.1.max(1));
        let overflow = full_visual_lines.len().saturating_sub(viewport_rows);

        if overflow > self.flushed_visual_count {
            let new_scrollback = &full_visual_lines[self.flushed_visual_count..overflow];
            self.restore_cursor_position()?;
            self.screen
                .push_to_scrollback(new_scrollback, &mut self.writer)?;
            self.flushed_visual_count = overflow;
        }

        let effective_flush = self.flushed_visual_count.min(overflow);
        let visual_lines = &full_visual_lines[effective_flush..];
        cursor_row = cursor_row.saturating_sub(effective_flush);
        if cursor_row >= visual_lines.len() {
            cursor_row = visual_lines.len().saturating_sub(1);
        }

        self.restore_cursor_position()?;
        self.screen
            .render(visual_lines, self.context.size.0, &mut self.writer)?;

        // Show or hide the cursor based on the component's request.
        if output.cursor_visible != self.cursor_visible {
            if output.cursor_visible {
                self.writer.queue(Show)?;
            } else {
                self.writer.queue(Hide)?;
            }
            self.cursor_visible = output.cursor_visible;
        }

        let rows_up = u16::try_from(
            visual_lines
                .len()
                .saturating_sub(1)
                .saturating_sub(cursor_row),
        )
        .unwrap_or(u16::MAX);
        self.reposition_cursor(rows_up, u16::try_from(cursor_col).unwrap_or(u16::MAX))?;
        Ok(())
    }

    /// Commit lines to permanent scrollback, replacing the managed region.
    ///
    /// Lines that were already progressively flushed during `render()` are
    /// skipped to avoid duplication in the terminal transcript.
    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()> {
        self.restore_cursor_position()?;

        let width = self.context.size.0;
        let visual: Vec<Line> = lines
            .iter()
            .flat_map(|l| soft_wrap_line(l, width))
            .collect();

        let remaining = &visual[self.flushed_visual_count.min(visual.len())..];
        self.screen
            .push_to_scrollback(remaining, &mut self.writer)?;

        self.flushed_visual_count = 0;
        Ok(())
    }

    /// Move the cursor to an absolute position relative to the end of the frame.
    ///
    /// `rows_up`: how many rows above the last frame line to place the cursor.
    /// `col`: column to move to (0-based, after a `\r`).
    pub fn reposition_cursor(&mut self, rows_up: u16, col: u16) -> io::Result<()> {
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

    pub fn update_render_context(&mut self) {
        let size = match crossterm::terminal::size() {
            Ok(size) => size,
            Err(e) => {
                eprintln!("Failed to get size: {e}");
                (80, 24)
            }
        };
        self.context.size = size;
    }

    pub fn update_render_context_with(&mut self, size: (u16, u16)) {
        self.context.size = size;
    }

    pub fn context(&self) -> &RenderContext {
        &self.context
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.context.theme = theme;
    }

    #[allow(dead_code)]
    pub fn writer(&self) -> &T {
        &self.writer
    }

    fn restore_cursor_position(&mut self) -> io::Result<()> {
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
    use crate::settings::WISP_HOME_ENV_MUTEX;
    use crossterm::style::Color;

    #[test]
    fn set_theme_replaces_render_context_theme() {
        let mut renderer = Renderer::new(Vec::new(), Theme::default());
        let new_theme = Theme::default();
        let expected = new_theme.text_primary();

        renderer.set_theme(new_theme);

        assert_eq!(renderer.context().theme.text_primary(), expected);
    }

    #[test]
    fn set_theme_replaces_render_context_theme_from_non_default_file() {
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
        let themes_dir = temp_dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("custom.tmTheme"), custom_tmtheme).unwrap();

        let _guard = WISP_HOME_ENV_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let old = std::env::var_os("WISP_HOME");
        unsafe { std::env::set_var("WISP_HOME", temp_dir.path()) };

        let settings = crate::settings::WispSettings {
            theme: crate::settings::ThemeSettings {
                file: Some("custom.tmTheme".to_string()),
            },
        };
        let loaded = Theme::load(&settings);
        renderer.set_theme(loaded);

        if let Some(value) = old {
            unsafe { std::env::set_var("WISP_HOME", value) };
        } else {
            unsafe { std::env::remove_var("WISP_HOME") };
        }

        assert_eq!(
            renderer.context().theme.text_primary(),
            Color::Rgb {
                r: 0x11,
                g: 0x22,
                b: 0x33
            }
        );
    }
}
