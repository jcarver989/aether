use std::io::{self, Write};
use std::sync::Arc;

use super::frame::Frame;
use super::line::Line;
use super::render_context::ViewContext;
use super::size::Size;
use super::terminal_screen::TerminalScreen;
use crate::theme::Theme;

#[cfg(feature = "syntax")]
use crate::syntax_highlighting::SyntaxHighlighter;

/// Pure TUI renderer that owns current render configuration.
pub struct Renderer<T: Write> {
    terminal: TerminalScreen<T>,
    size: Size,
    theme: Arc<Theme>,
    #[cfg(feature = "syntax")]
    highlighter: Arc<SyntaxHighlighter>,
    render_epoch: u64,
}

impl<T: Write> Renderer<T> {
    pub fn new(writer: T, theme: Theme) -> Self {
        Self {
            terminal: TerminalScreen::new(writer),
            size: (0, 0).into(),
            theme: Arc::new(theme),
            #[cfg(feature = "syntax")]
            highlighter: Arc::new(SyntaxHighlighter::new()),
            render_epoch: 0,
        }
    }

    /// Render a frame using a closure.
    ///
    /// The closure receives a ViewContext and returns a Frame.
    pub fn render_frame(&mut self, f: impl FnOnce(&ViewContext) -> Frame) -> io::Result<()> {
        let context = self.context();
        let prepared = f(&context)
            .soft_wrap(self.size.width)
            .clamp_cursor()
            .prepare(self.size, self.terminal.flushed_visual_count());
        self.terminal.render_frame(&prepared, self.size.width)
    }

    pub fn clear_screen(&mut self) -> io::Result<()> {
        self.bump_render_epoch();
        self.terminal.clear_screen()
    }

    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()> {
        self.bump_render_epoch();
        self.terminal.push_to_scrollback(lines, self.size.width)
    }

    pub fn on_resize(&mut self, size: impl Into<Size>) {
        self.bump_render_epoch();
        self.size = size.into();
        self.terminal.on_resize(self.size.width);
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

    #[allow(dead_code)]
    pub fn writer(&self) -> &T {
        self.terminal.writer()
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn test_writer_mut(&mut self) -> &mut T {
        self.terminal.writer_mut()
    }

    pub fn render_epoch(&self) -> u64 {
        self.render_epoch
    }

    fn bump_render_epoch(&mut self) {
        self.render_epoch = self.render_epoch.wrapping_add(1);
    }

    /// Create a narrowed [`Terminal`] handle for use in effect handlers.
    pub fn terminal(&mut self) -> Terminal<'_, T> {
        Terminal { renderer: self }
    }
}

/// Narrowed handle for terminal operations during effect execution.
///
/// Provides only the operations needed by effect handlers: scrollback,
/// screen clearing, theme changes, and context queries.
pub struct Terminal<'a, W: Write> {
    renderer: &'a mut Renderer<W>,
}

impl<W: Write> Terminal<'_, W> {
    pub fn push_to_scrollback(&mut self, lines: &[Line]) -> io::Result<()> {
        self.renderer.push_to_scrollback(lines)
    }

    pub fn clear_screen(&mut self) -> io::Result<()> {
        self.renderer.clear_screen()
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.renderer.set_theme(theme);
    }

    pub fn context(&self) -> ViewContext {
        self.renderer.context()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::style::Color;

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
            Color::Rgb {
                r: 0x11,
                g: 0x22,
                b: 0x33
            }
        );
    }
}
