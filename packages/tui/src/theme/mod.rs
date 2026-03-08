#[cfg(feature = "syntax")]
use std::sync::Arc;

use crate::style::Style;
use crossterm::style::Color;
mod defaults;

#[cfg(feature = "syntax")]
mod syntax;

/// Semantic color palette for TUI rendering.
#[derive(Clone, Debug)]
pub struct ColorPalette {
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
}

#[derive(Clone, Debug, Default)]
pub struct ColorPaletteBuilder {
    fg: Option<Color>,
    bg: Option<Color>,
    accent: Option<Color>,
    highlight_bg: Option<Color>,
    text_secondary: Option<Color>,
    code_fg: Option<Color>,
    code_bg: Option<Color>,
    heading: Option<Color>,
    link: Option<Color>,
    blockquote: Option<Color>,
    muted: Option<Color>,
    success: Option<Color>,
    warning: Option<Color>,
    error: Option<Color>,
    info: Option<Color>,
    secondary: Option<Color>,
    diff_added_fg: Option<Color>,
    diff_removed_fg: Option<Color>,
    diff_added_bg: Option<Color>,
    diff_removed_bg: Option<Color>,
}

/// Full resolved theme for TUI rendering.
///
/// Owns the semantic color palette used throughout the UI and, when enabled,
/// the cached syntax-highlighting theme.
#[derive(Clone, Debug)]
pub struct Theme {
    palette: ColorPalette,

    // Cached syntect theme for syntax highlighting (parsed once at construction)
    #[cfg(feature = "syntax")]
    #[allow(clippy::struct_field_names)]
    syntect_theme: Arc<syntect::highlighting::Theme>,
}

impl ColorPaletteBuilder {
    pub fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    pub fn accent(mut self, color: Color) -> Self {
        self.accent = Some(color);
        self
    }

    pub fn highlight_bg(mut self, color: Color) -> Self {
        self.highlight_bg = Some(color);
        self
    }

    pub fn text_secondary(mut self, color: Color) -> Self {
        self.text_secondary = Some(color);
        self
    }

    pub fn code_fg(mut self, color: Color) -> Self {
        self.code_fg = Some(color);
        self
    }

    pub fn code_bg(mut self, color: Color) -> Self {
        self.code_bg = Some(color);
        self
    }

    pub fn heading(mut self, color: Color) -> Self {
        self.heading = Some(color);
        self
    }

    pub fn link(mut self, color: Color) -> Self {
        self.link = Some(color);
        self
    }

    pub fn blockquote(mut self, color: Color) -> Self {
        self.blockquote = Some(color);
        self
    }

    pub fn muted(mut self, color: Color) -> Self {
        self.muted = Some(color);
        self
    }

    pub fn success(mut self, color: Color) -> Self {
        self.success = Some(color);
        self
    }

    pub fn warning(mut self, color: Color) -> Self {
        self.warning = Some(color);
        self
    }

    pub fn error(mut self, color: Color) -> Self {
        self.error = Some(color);
        self
    }

    pub fn info(mut self, color: Color) -> Self {
        self.info = Some(color);
        self
    }

    pub fn secondary(mut self, color: Color) -> Self {
        self.secondary = Some(color);
        self
    }

    pub fn diff_added_fg(mut self, color: Color) -> Self {
        self.diff_added_fg = Some(color);
        self
    }

    pub fn diff_removed_fg(mut self, color: Color) -> Self {
        self.diff_removed_fg = Some(color);
        self
    }

    pub fn diff_added_bg(mut self, color: Color) -> Self {
        self.diff_added_bg = Some(color);
        self
    }

    pub fn diff_removed_bg(mut self, color: Color) -> Self {
        self.diff_removed_bg = Some(color);
        self
    }

    pub fn build(self) -> ColorPalette {
        ColorPalette {
            fg: self.fg.expect("ColorPaletteBuilder requires fg"),
            bg: self.bg.expect("ColorPaletteBuilder requires bg"),
            accent: self.accent.expect("ColorPaletteBuilder requires accent"),
            highlight_bg: self
                .highlight_bg
                .expect("ColorPaletteBuilder requires highlight_bg"),
            text_secondary: self
                .text_secondary
                .expect("ColorPaletteBuilder requires text_secondary"),
            code_fg: self.code_fg.expect("ColorPaletteBuilder requires code_fg"),
            code_bg: self.code_bg.expect("ColorPaletteBuilder requires code_bg"),
            heading: self.heading.expect("ColorPaletteBuilder requires heading"),
            link: self.link.expect("ColorPaletteBuilder requires link"),
            blockquote: self
                .blockquote
                .expect("ColorPaletteBuilder requires blockquote"),
            muted: self.muted.expect("ColorPaletteBuilder requires muted"),
            success: self.success.expect("ColorPaletteBuilder requires success"),
            warning: self.warning.expect("ColorPaletteBuilder requires warning"),
            error: self.error.expect("ColorPaletteBuilder requires error"),
            info: self.info.expect("ColorPaletteBuilder requires info"),
            secondary: self
                .secondary
                .expect("ColorPaletteBuilder requires secondary"),
            diff_added_fg: self
                .diff_added_fg
                .expect("ColorPaletteBuilder requires diff_added_fg"),
            diff_removed_fg: self
                .diff_removed_fg
                .expect("ColorPaletteBuilder requires diff_removed_fg"),
            diff_added_bg: self
                .diff_added_bg
                .expect("ColorPaletteBuilder requires diff_added_bg"),
            diff_removed_bg: self
                .diff_removed_bg
                .expect("ColorPaletteBuilder requires diff_removed_bg"),
        }
    }
}

#[allow(dead_code, clippy::unused_self)]
impl ColorPalette {
    pub fn builder() -> ColorPaletteBuilder {
        ColorPaletteBuilder::default()
    }

    pub fn primary(&self) -> Color {
        self.fg
    }

    pub fn text_primary(&self) -> Color {
        self.fg
    }

    pub fn background(&self) -> Color {
        self.bg
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
}

#[allow(dead_code, clippy::unused_self)]
impl Theme {
    pub(crate) fn from_palette(palette: ColorPalette) -> Self {
        Self {
            palette,
            #[cfg(feature = "syntax")]
            syntect_theme: Arc::new(syntax::parse_default_syntect_theme()),
        }
    }

    pub fn new(palette: ColorPalette) -> Self {
        Self::from_palette(palette)
    }

    pub fn palette(&self) -> &ColorPalette {
        &self.palette
    }

    pub fn primary(&self) -> Color {
        self.palette.primary()
    }

    pub fn text_primary(&self) -> Color {
        self.palette.text_primary()
    }

    pub fn background(&self) -> Color {
        self.palette.background()
    }

    pub fn code_fg(&self) -> Color {
        self.palette.code_fg()
    }

    pub fn code_bg(&self) -> Color {
        self.palette.code_bg()
    }

    pub fn accent(&self) -> Color {
        self.palette.accent()
    }

    pub fn highlight_bg(&self) -> Color {
        self.palette.highlight_bg()
    }

    pub fn selected_row_style(&self) -> Style {
        self.palette.selected_row_style()
    }

    pub fn selected_row_style_with_fg(&self, fg: Color) -> Style {
        self.palette.selected_row_style_with_fg(fg)
    }

    pub fn secondary(&self) -> Color {
        self.palette.secondary()
    }

    pub fn text_secondary(&self) -> Color {
        self.palette.text_secondary()
    }

    pub fn success(&self) -> Color {
        self.palette.success()
    }

    pub fn warning(&self) -> Color {
        self.palette.warning()
    }

    pub fn error(&self) -> Color {
        self.palette.error()
    }

    pub fn info(&self) -> Color {
        self.palette.info()
    }

    pub fn muted(&self) -> Color {
        self.palette.muted()
    }

    pub fn heading(&self) -> Color {
        self.palette.heading()
    }

    pub fn link(&self) -> Color {
        self.palette.link()
    }

    pub fn blockquote(&self) -> Color {
        self.palette.blockquote()
    }

    pub fn diff_added_bg(&self) -> Color {
        self.palette.diff_added_bg()
    }

    pub fn diff_removed_bg(&self) -> Color {
        self.palette.diff_removed_bg()
    }

    pub fn diff_added_fg(&self) -> Color {
        self.palette.diff_added_fg()
    }

    pub fn diff_removed_fg(&self) -> Color {
        self.palette.diff_removed_fg()
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
    fn palette_exposes_theme_semantic_colors() {
        let theme = Theme::default();
        assert_eq!(theme.palette().text_primary(), theme.text_primary());
        assert_eq!(theme.palette().background(), theme.background());
        assert_eq!(theme.palette().muted(), theme.muted());
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
            ColorPalette::builder()
                .fg(Color::Black)
                .bg(Color::White)
                .accent(Color::Red)
                .highlight_bg(Color::Green)
                .text_secondary(Color::Yellow)
                .code_fg(Color::Blue)
                .code_bg(Color::Magenta)
                .heading(Color::Cyan)
                .link(Color::DarkGrey)
                .blockquote(Color::DarkRed)
                .muted(Color::DarkGreen)
                .success(Color::DarkBlue)
                .warning(Color::DarkCyan)
                .error(Color::DarkMagenta)
                .info(Color::Grey)
                .secondary(Color::Rgb {
                    r: 128,
                    g: 0,
                    b: 128,
                })
                .diff_added_fg(Color::Rgb { r: 0, g: 255, b: 0 })
                .diff_removed_fg(Color::Rgb { r: 255, g: 0, b: 0 })
                .diff_added_bg(Color::Rgb { r: 0, g: 20, b: 0 })
                .diff_removed_bg(Color::Rgb { r: 20, g: 0, b: 0 })
                .build(),
        );
        assert_eq!(theme.primary(), Color::Black);
        assert_eq!(theme.background(), Color::White);
        assert_eq!(theme.accent(), Color::Red);
    }
}
