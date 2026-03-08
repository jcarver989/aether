#[cfg(feature = "syntax")]
use std::sync::Arc;

use crate::style::Style;
use crossterm::style::Color;
mod defaults;

#[cfg(feature = "syntax")]
mod syntax;

/// Semantic color palette for TUI rendering.
///
/// Provides semantic color accessors for consistent theming across components.
#[derive(Clone, Debug)]
pub struct Theme {
    // Base colors
    fg: Color,
    bg: Color,
    accent: Color,
    highlight_bg: Color,

    // Text colors
    text_secondary: Color,
    code_fg: Color,
    code_bg: Color,

    // Markdown semantic colors
    heading: Color,
    link: Color,
    blockquote: Color,
    muted: Color,

    // Status colors
    success: Color,
    warning: Color,
    error: Color,
    info: Color,
    secondary: Color,

    // Diff colors
    diff_added_fg: Color,
    diff_removed_fg: Color,
    diff_added_bg: Color,
    diff_removed_bg: Color,

    // Cached syntect theme for syntax highlighting (parsed once at construction)
    #[cfg(feature = "syntax")]
    #[allow(clippy::struct_field_names)]
    syntect_theme: Arc<syntect::highlighting::Theme>,
}

#[allow(dead_code, clippy::unused_self)]
impl Theme {
    pub fn primary(&self) -> Color {
        self.fg
    }

    pub fn text_primary(&self) -> Color {
        self.fg
    }

    pub fn code_fg(&self) -> Color {
        self.code_fg
    }

    pub fn code_bg(&self) -> Color {
        self.code_bg
    }

    pub fn accent(&self) -> Color {
        self.accent
    }

    pub fn highlight_bg(&self) -> Color {
        self.highlight_bg
    }

    pub fn selected_row_style(&self) -> Style {
        self.selected_row_style_with_fg(self.text_primary())
    }

    pub fn selected_row_style_with_fg(&self, fg: Color) -> Style {
        Style::fg(fg).bg_color(self.highlight_bg())
    }

    pub fn secondary(&self) -> Color {
        self.secondary
    }

    pub fn text_secondary(&self) -> Color {
        self.text_secondary
    }

    pub fn success(&self) -> Color {
        self.success
    }

    pub fn warning(&self) -> Color {
        self.warning
    }

    pub fn error(&self) -> Color {
        self.error
    }

    pub fn info(&self) -> Color {
        self.info
    }

    pub fn muted(&self) -> Color {
        self.muted
    }

    pub fn heading(&self) -> Color {
        self.heading
    }

    pub fn link(&self) -> Color {
        self.link
    }

    pub fn blockquote(&self) -> Color {
        self.blockquote
    }

    pub fn diff_added_bg(&self) -> Color {
        self.diff_added_bg
    }

    pub fn diff_removed_bg(&self) -> Color {
        self.diff_removed_bg
    }

    pub fn diff_added_fg(&self) -> Color {
        self.diff_added_fg
    }

    pub fn diff_removed_fg(&self) -> Color {
        self.diff_removed_fg
    }

    /// Create a custom theme with specific colors.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fg: Color,
        bg: Color,
        accent: Color,
        highlight_bg: Color,
        text_secondary: Color,
        code_fg: Color,
        code_bg: Color,
        heading: Color,
        link: Color,
        blockquote: Color,
        muted: Color,
        success: Color,
        warning: Color,
        error: Color,
        info: Color,
        secondary: Color,
        diff_added_fg: Color,
        diff_removed_fg: Color,
        diff_added_bg: Color,
        diff_removed_bg: Color,
    ) -> Self {
        Self {
            fg,
            bg,
            accent,
            highlight_bg,
            text_secondary,
            code_fg,
            code_bg,
            heading,
            link,
            blockquote,
            muted,
            success,
            warning,
            error,
            info,
            secondary,
            diff_added_fg,
            diff_removed_fg,
            diff_added_bg,
            diff_removed_bg,
            #[cfg(feature = "syntax")]
            syntect_theme: Arc::new(syntax::parse_default_syntect_theme()),
        }
    }
}

/// Darken a color to ~20% brightness for use as a subtle background.
#[allow(clippy::cast_possible_truncation)]
fn darken_color(color: Color) -> Color {
    match color {
        Color::Rgb { r, g, b } => Color::Rgb {
            r: (u16::from(r) * 20 / 100) as u8,
            g: (u16::from(g) * 20 / 100) as u8,
            b: (u16::from(b) * 20 / 100) as u8,
        },
        other => other,
    }
}

/// Lighten a color to ~10% brightness for use as a subtle background.
#[allow(clippy::cast_possible_truncation)]
#[allow(dead_code)]
fn lighten_color(color: Color) -> Color {
    match color {
        Color::Rgb { r, g, b } => Color::Rgb {
            r: (u16::from(r) * 10 / 100 + 230) as u8,
            g: (u16::from(g) * 10 / 100 + 230) as u8,
            b: (u16::from(b) * 10 / 100 + 230) as u8,
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_row_style_uses_text_primary_and_highlight_bg() {
        let theme = Theme::default();
        let style = theme.selected_row_style();
        assert_eq!(style.fg, Some(theme.text_primary()));
        assert_eq!(style.bg, Some(theme.highlight_bg()));
    }

    #[test]
    fn selected_row_style_with_fg_preserves_custom_foreground() {
        let theme = Theme::default();
        let style = theme.selected_row_style_with_fg(theme.warning());
        assert_eq!(style.fg, Some(theme.warning()));
        assert_eq!(style.bg, Some(theme.highlight_bg()));
    }

    #[test]
    fn code_fg_differs_from_text_primary() {
        let theme = Theme::default();
        assert_ne!(
            theme.code_fg(),
            theme.text_primary(),
            "code_fg should be visually distinct from body text"
        );
    }

    #[test]
    fn darken_color_reduces_brightness() {
        let bright = Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        };
        let dark = darken_color(bright);
        assert_eq!(
            dark,
            Color::Rgb {
                r: 40,
                g: 20,
                b: 10
            }
        );
    }

    #[test]
    fn custom_theme_builder() {
        let theme = Theme::new(
            Color::Black,       // fg
            Color::White,       // bg
            Color::Red,         // accent
            Color::Green,       // highlight_bg
            Color::Yellow,      // text_secondary
            Color::Blue,        // code_fg
            Color::Magenta,     // code_bg
            Color::Cyan,        // heading
            Color::DarkGrey,    // link
            Color::DarkRed,     // blockquote
            Color::DarkGreen,   // muted
            Color::DarkBlue,    // success
            Color::DarkCyan,    // warning
            Color::DarkMagenta, // error
            Color::Grey,        // info
            Color::Rgb {
                r: 128,
                g: 0,
                b: 128,
            }, // secondary
            Color::Rgb { r: 0, g: 255, b: 0 }, // diff_added_fg
            Color::Rgb { r: 255, g: 0, b: 0 }, // diff_removed_fg
            Color::Rgb { r: 0, g: 20, b: 0 }, // diff_added_bg
            Color::Rgb { r: 20, g: 0, b: 0 }, // diff_removed_bg
        );
        assert_eq!(theme.primary(), Color::Black);
        assert_eq!(theme.accent(), Color::Red);
    }
}
