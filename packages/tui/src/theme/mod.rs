use crate::style::Style;
use crossterm::style::Color;

/// Semantic color palette for TUI rendering.
///
/// Cheap to clone (all fields are Copy). Provides semantic color accessors
/// for consistent theming across components.
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
}

impl Default for Theme {
    fn default() -> Self {
        Self::catppuccin_mocha()
    }
}

#[allow(dead_code, clippy::unused_self)]
impl Theme {
    /// Catppuccin Mocha theme (dark, high contrast).
    ///
    /// This is the default theme and is always available without any
    /// additional dependencies.
    pub fn catppuccin_mocha() -> Self {
        // Catppuccin Mocha palette
        const TEXT: Color = Color::Rgb {
            r: 0xCD,
            g: 0xD6,
            b: 0xF4,
        };
        const BASE: Color = Color::Rgb {
            r: 0x1E,
            g: 0x1E,
            b: 0x2E,
        };
        const ROSEWATER: Color = Color::Rgb {
            r: 0xF5,
            g: 0xE0,
            b: 0xDC,
        };
        const YELLOW: Color = Color::Rgb {
            r: 0xF9,
            g: 0xE2,
            b: 0xAF,
        };
        const BLUE: Color = Color::Rgb {
            r: 0x89,
            g: 0xB4,
            b: 0xFA,
        };
        const PINK: Color = Color::Rgb {
            r: 0xF5,
            g: 0xC2,
            b: 0xE7,
        };
        const TEAL: Color = Color::Rgb {
            r: 0x94,
            g: 0xE2,
            b: 0xD5,
        };
        const GREEN: Color = Color::Rgb {
            r: 0xA6,
            g: 0xE3,
            b: 0xA1,
        };
        const PEACH: Color = Color::Rgb {
            r: 0xFA,
            g: 0xB3,
            b: 0x87,
        };
        const RED: Color = Color::Rgb {
            r: 0xF3,
            g: 0x8B,
            b: 0xA8,
        };
        const MAUVE: Color = Color::Rgb {
            r: 0xCB,
            g: 0xA6,
            b: 0xF7,
        };
        const SURFACE0: Color = Color::Rgb {
            r: 0x31,
            g: 0x31,
            b: 0x44,
        };
        const SURFACE1: Color = Color::Rgb {
            r: 0x45,
            g: 0x45,
            b: 0x45,
        };
        const OVERLAY0: Color = Color::Rgb {
            r: 0x6C,
            g: 0x70,
            b: 0x85,
        };

        Self {
            fg: TEXT,
            bg: BASE,
            accent: ROSEWATER,
            highlight_bg: Color::Rgb {
                r: 0x31,
                g: 0x4A,
                b: 0x56,
            },

            text_secondary: OVERLAY0,
            code_fg: GREEN,
            code_bg: SURFACE1,

            heading: YELLOW,
            link: BLUE,
            blockquote: PINK,
            muted: TEAL,

            success: GREEN,
            warning: PEACH,
            error: RED,
            info: BLUE,
            secondary: MAUVE,

            diff_added_fg: GREEN,
            diff_removed_fg: RED,
            diff_added_bg: darken_color(GREEN),
            diff_removed_bg: darken_color(RED),
        }
    }

    /// A minimal light theme.
    #[allow(dead_code)]
    pub fn light() -> Self {
        const FG: Color = Color::Rgb {
            r: 0x22,
            g: 0x22,
            b: 0x22,
        };
        const BG: Color = Color::Rgb {
            r: 0xFA,
            g: 0xFA,
            b: 0xFA,
        };
        const ACCENT: Color = Color::Rgb {
            r: 0x00,
            g: 0x66,
            b: 0xCC,
        };
        const GREEN: Color = Color::Rgb {
            r: 0x22,
            g: 0x88,
            b: 0x22,
        };
        const RED: Color = Color::Rgb {
            r: 0xCC,
            g: 0x22,
            b: 0x22,
        };
        const ORANGE: Color = Color::Rgb {
            r: 0xCC,
            g: 0x66,
            b: 0x00,
        };

        Self {
            fg: FG,
            bg: BG,
            accent: ACCENT,
            highlight_bg: Color::Rgb {
                r: 0xDD,
                g: 0xDD,
                b: 0xDD,
            },

            text_secondary: Color::Rgb {
                r: 0x66,
                g: 0x66,
                b: 0x66,
            },
            code_fg: Color::Rgb {
                r: 0x33,
                g: 0x66,
                b: 0x33,
            },
            code_bg: Color::Rgb {
                r: 0xF0,
                g: 0xF0,
                b: 0xF0,
            },

            heading: ACCENT,
            link: Color::Rgb {
                r: 0x00,
                g: 0x44,
                b: 0xAA,
            },
            blockquote: Color::Rgb {
                r: 0x66,
                g: 0x44,
                b: 0x88,
            },
            muted: Color::Rgb {
                r: 0x88,
                g: 0x88,
                b: 0x88,
            },

            success: GREEN,
            warning: ORANGE,
            error: RED,
            info: ACCENT,
            secondary: Color::Rgb {
                r: 0x66,
                g: 0x33,
                b: 0x99,
            },

            diff_added_fg: GREEN,
            diff_removed_fg: RED,
            diff_added_bg: lighten_color(GREEN),
            diff_removed_bg: lighten_color(RED),
        }
    }

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
        }
    }
}

#[cfg(feature = "syntax")]
mod syntax {
    use super::*;
    use std::path::Path;
    use std::sync::{Arc, LazyLock};

    /// Embedded Catppuccin Mocha theme for syntax highlighting.
    static DEFAULT_SYNTAX_THEME: LazyLock<Arc<syntect::highlighting::Theme>> =
        LazyLock::new(|| {
            let cursor =
                std::io::Cursor::new(include_bytes!("../../assets/catppuccin-mocha.tmTheme"));
            let theme = syntect::highlighting::ThemeSet::load_from_reader(
                &mut std::io::BufReader::new(cursor),
            )
            .expect("embedded catppuccin-mocha.tmTheme is valid");
            Arc::new(theme)
        });

    /// Trait for syntect integration when the `syntax` feature is enabled.
    impl Theme {
        /// Get a reference to the underlying syntect theme for syntax highlighting.
        ///
        /// This is used by the markdown and diff modules for syntax highlighting.
        pub fn syntect_theme(&self) -> &syntect::highlighting::Theme {
            // Return the embedded Catppuccin theme for syntax highlighting.
            // This is used by HighlightLines which needs a syntect theme.
            &DEFAULT_SYNTAX_THEME
        }

        /// Load theme from a `.tmTheme` file.
        pub fn load_from_path(path: &Path) -> Self {
            use syntect::highlighting::ThemeSet;
            use tracing::warn;

            match ThemeSet::get_theme(path) {
                Ok(syntect_theme) => Self::from(&syntect_theme),
                Err(e) => {
                    warn!(
                        "Failed to load theme from {}: {e}. Falling back to defaults.",
                        path.display()
                    );
                    Self::default()
                }
            }
        }
    }

    impl From<&syntect::highlighting::Theme> for Theme {
        fn from(syntect: &syntect::highlighting::Theme) -> Self {
            let accent = syntect
                .settings
                .caret
                .map_or(super::DEFAULT_ACCENT, color_from_syntect);

            let text_secondary = derive_text_secondary(syntect);

            let heading = resolve_scope_fg(syntect, "markup.heading.markdown")
                .or_else(|| resolve_scope_fg(syntect, "markup.heading"))
                .unwrap_or(accent);

            let link = resolve_scope_fg(syntect, "markup.underline.link")
                .or_else(|| resolve_scope_fg(syntect, "markup.link"))
                .unwrap_or(accent);

            let blockquote = resolve_scope_fg(syntect, "markup.quote").unwrap_or(text_secondary);

            let muted = resolve_scope_fg(syntect, "markup.list.bullet")
                .or_else(|| syntect.settings.gutter_foreground.map(color_from_syntect))
                .unwrap_or(text_secondary);

            let fg = syntect
                .settings
                .foreground
                .map_or(super::DEFAULT_FG, color_from_syntect);

            let code_fg = resolve_scope_fg(syntect, "markup.inline.raw.string.markdown")
                .or_else(|| resolve_scope_fg(syntect, "markup.raw"))
                .unwrap_or(fg);

            let error = resolve_scope_fg(syntect, "markup.deleted")
                .or_else(|| resolve_scope_fg(syntect, "markup.deleted.diff"))
                .or_else(|| resolve_scope_fg(syntect, "invalid"))
                .unwrap_or(accent);

            let warning = resolve_scope_fg(syntect, "constant.numeric").unwrap_or(accent);

            let success = resolve_scope_fg(syntect, "markup.inserted")
                .or_else(|| resolve_scope_fg(syntect, "markup.inserted.diff"))
                .or_else(|| resolve_scope_fg(syntect, "string"))
                .unwrap_or(accent);

            let info = resolve_scope_fg(syntect, "entity.name.function")
                .or_else(|| resolve_scope_fg(syntect, "support.function"))
                .unwrap_or(accent);

            let secondary = resolve_scope_fg(syntect, "keyword")
                .or_else(|| resolve_scope_fg(syntect, "storage.type"))
                .unwrap_or(accent);

            let diff_added_fg = resolve_scope_fg(syntect, "markup.inserted.diff")
                .or_else(|| resolve_scope_fg(syntect, "markup.inserted"))
                .or_else(|| resolve_scope_fg(syntect, "string"))
                .unwrap_or(accent);

            let diff_removed_fg = resolve_scope_fg(syntect, "markup.deleted.diff")
                .or_else(|| resolve_scope_fg(syntect, "markup.deleted"))
                .unwrap_or(accent);

            let highlight_bg = syntect
                .settings
                .selection
                .map_or(super::DEFAULT_HIGHLIGHT_BG, color_from_syntect);

            let bg = syntect
                .settings
                .background
                .map_or(super::DEFAULT_BG, color_from_syntect);

            let code_bg = syntect
                .settings
                .background
                .map_or(super::DEFAULT_CODE_BG, color_from_syntect);

            let diff_added_bg = darken_color(diff_added_fg);
            let diff_removed_bg = darken_color(diff_removed_fg);

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
            }
        }
    }

    /// Resolve the foreground color for a scope string against the theme.
    fn resolve_scope_fg(theme: &syntect::highlighting::Theme, scope_str: &str) -> Option<Color> {
        use syntect::highlighting::Highlighter;
        use syntect::parsing::Scope;

        let scope = Scope::new(scope_str).ok()?;
        let highlighter = Highlighter::new(theme);
        let style = highlighter.style_for_stack(&[scope]);

        let resolved = style.foreground;
        let default_fg = theme.settings.foreground?;

        if resolved.r == default_fg.r && resolved.g == default_fg.g && resolved.b == default_fg.b {
            return None;
        }

        Some(color_from_syntect(resolved))
    }

    /// Blend the theme's foreground toward its background at ~40%.
    fn derive_text_secondary(theme: &syntect::highlighting::Theme) -> Color {
        use syntect::highlighting::Color as SyntectColor;

        let fg = theme.settings.foreground.unwrap_or(SyntectColor {
            r: 0xBF,
            g: 0xBD,
            b: 0xB6,
            a: 0xFF,
        });
        let bg = theme.settings.background.unwrap_or(SyntectColor {
            r: 0x28,
            g: 0x28,
            b: 0x28,
            a: 0xFF,
        });

        #[allow(clippy::cast_possible_truncation)]
        let blend = |f: u8, b: u8| -> u8 { ((u16::from(f) * 60 + u16::from(b) * 40) / 100) as u8 };

        Color::Rgb {
            r: blend(fg.r, bg.r),
            g: blend(fg.g, bg.g),
            b: blend(fg.b, bg.b),
        }
    }

    fn color_from_syntect(color: syntect::highlighting::Color) -> Color {
        Color::Rgb {
            r: color.r,
            g: color.g,
            b: color.b,
        }
    }
}

#[cfg(feature = "syntax")]
const DEFAULT_FG: Color = Color::Rgb {
    r: 0xBF,
    g: 0xBD,
    b: 0xB6,
};
#[cfg(feature = "syntax")]
const DEFAULT_BG: Color = Color::Rgb {
    r: 0x1E,
    g: 0x1E,
    b: 0x2E,
};
#[cfg(feature = "syntax")]
const DEFAULT_CODE_BG: Color = Color::Rgb {
    r: 40,
    g: 40,
    b: 40,
};
#[cfg(feature = "syntax")]
const DEFAULT_ACCENT: Color = Color::Rgb {
    r: 255,
    g: 215,
    b: 0,
};
#[cfg(feature = "syntax")]
const DEFAULT_HIGHLIGHT_BG: Color = Color::Rgb {
    r: 0x1a,
    g: 0x4a,
    b: 0x50,
};

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

    /// Catppuccin Mocha palette constants.
    const CATPPUCCIN_HEADING: Color = Color::Rgb {
        r: 0xf9,
        g: 0xe2,
        b: 0xaf,
    }; // yellow
    const CATPPUCCIN_LINK: Color = Color::Rgb {
        r: 0x89,
        g: 0xb4,
        b: 0xfa,
    }; // blue
    const CATPPUCCIN_BLOCKQUOTE: Color = Color::Rgb {
        r: 0xf5,
        g: 0xc2,
        b: 0xe7,
    }; // pink
    const CATPPUCCIN_MUTED: Color = Color::Rgb {
        r: 0x94,
        g: 0xe2,
        b: 0xd5,
    }; // teal
    const CATPPUCCIN_CODE_FG: Color = Color::Rgb {
        r: 0xa6,
        g: 0xe3,
        b: 0xa1,
    }; // green
    const CATPPUCCIN_ERROR: Color = Color::Rgb {
        r: 0xf3,
        g: 0x8b,
        b: 0xa8,
    }; // red
    const CATPPUCCIN_WARNING: Color = Color::Rgb {
        r: 0xfa,
        g: 0xb3,
        b: 0x87,
    }; // peach
    const CATPPUCCIN_SUCCESS: Color = Color::Rgb {
        r: 0xa6,
        g: 0xe3,
        b: 0xa1,
    }; // green
    const CATPPUCCIN_INFO: Color = Color::Rgb {
        r: 0x89,
        g: 0xb4,
        b: 0xfa,
    }; // blue
    const CATPPUCCIN_SECONDARY: Color = Color::Rgb {
        r: 0xcb,
        g: 0xa6,
        b: 0xf7,
    }; // mauve
    const CATPPUCCIN_ACCENT: Color = Color::Rgb {
        r: 0xf5,
        g: 0xe0,
        b: 0xdc,
    }; // rosewater

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
    fn default_theme_is_catppuccin_mocha() {
        let theme = Theme::default();
        assert_eq!(theme.heading(), CATPPUCCIN_HEADING);
        assert_eq!(theme.link(), CATPPUCCIN_LINK);
        assert_eq!(theme.blockquote(), CATPPUCCIN_BLOCKQUOTE);
        assert_eq!(theme.code_fg(), CATPPUCCIN_CODE_FG);
        assert_eq!(theme.muted(), CATPPUCCIN_MUTED);
    }

    #[test]
    fn catppuccin_resolves_semantic_colors() {
        let theme = Theme::default();
        assert_eq!(theme.error(), CATPPUCCIN_ERROR);
        assert_eq!(theme.warning(), CATPPUCCIN_WARNING);
        assert_eq!(theme.success(), CATPPUCCIN_SUCCESS);
        assert_eq!(theme.info(), CATPPUCCIN_INFO);
        assert_eq!(theme.secondary(), CATPPUCCIN_SECONDARY);
    }

    #[test]
    fn catppuccin_resolves_diff_colors() {
        let theme = Theme::default();
        assert_eq!(theme.diff_added_fg(), CATPPUCCIN_SUCCESS);
        assert_eq!(theme.diff_removed_fg(), CATPPUCCIN_ERROR);
        assert_eq!(theme.diff_added_bg(), darken_color(CATPPUCCIN_SUCCESS));
        assert_eq!(theme.diff_removed_bg(), darken_color(CATPPUCCIN_ERROR));
    }

    #[test]
    fn catppuccin_accent_is_rosewater() {
        let theme = Theme::default();
        assert_eq!(theme.accent(), CATPPUCCIN_ACCENT);
    }

    #[test]
    fn catppuccin_code_fg_differs_from_text_primary() {
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

    #[cfg(feature = "syntax")]
    mod syntax_tests {
        use super::*;
        use std::fs;
        use syntect::highlighting::ThemeSettings;
        use tempfile::TempDir;

        fn bare_syntect_theme() -> std::sync::Arc<syntect::highlighting::Theme> {
            std::sync::Arc::new(syntect::highlighting::Theme {
                name: Some("Bare".into()),
                author: None,
                settings: ThemeSettings {
                    foreground: Some(syntect::highlighting::Color {
                        r: 0xCC,
                        g: 0xCC,
                        b: 0xCC,
                        a: 0xFF,
                    }),
                    background: Some(syntect::highlighting::Color {
                        r: 0x11,
                        g: 0x11,
                        b: 0x11,
                        a: 0xFF,
                    }),
                    caret: Some(syntect::highlighting::Color {
                        r: 0xAA,
                        g: 0xBB,
                        b: 0xCC,
                        a: 0xFF,
                    }),
                    ..ThemeSettings::default()
                },
                scopes: Vec::new(),
            })
        }

        const LOADABLE_TMTHEME: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>name</key>
    <string>Loadable</string>
    <key>settings</key>
    <array>
        <dict>
            <key>settings</key>
            <dict>
                <key>foreground</key>
                <string>#112233</string>
                <key>background</key>
                <string>#000000</string>
                <key>selection</key>
                <string>#334455</string>
            </dict>
        </dict>
    </array>
</dict>
</plist>"#;

        #[test]
        fn bare_theme_falls_back_to_accent() {
            let accent = Color::Rgb {
                r: 0xAA,
                g: 0xBB,
                b: 0xCC,
            };
            let syntect = bare_syntect_theme();
            let theme = Theme::from(&*syntect);

            assert_eq!(theme.heading(), accent);
            assert_eq!(theme.link(), accent);
            assert_eq!(theme.error(), accent);
            assert_eq!(theme.warning(), accent);
            assert_eq!(theme.success(), accent);
            assert_eq!(theme.info(), accent);
            assert_eq!(theme.secondary(), accent);
            assert_eq!(theme.diff_added_fg(), accent);
            assert_eq!(theme.diff_removed_fg(), accent);
        }

        #[test]
        fn valid_theme_file_loads_from_path() {
            let temp_dir = TempDir::new().unwrap();
            let theme_path = temp_dir.path().join("custom.tmTheme");
            fs::write(&theme_path, LOADABLE_TMTHEME).unwrap();

            let loaded = Theme::load_from_path(&theme_path);

            assert_eq!(
                loaded.text_primary(),
                Color::Rgb {
                    r: 0x11,
                    g: 0x22,
                    b: 0x33
                }
            );
        }

        #[test]
        fn malformed_theme_falls_back_to_default() {
            let temp_dir = TempDir::new().unwrap();
            let theme_path = temp_dir.path().join("broken.tmTheme");
            fs::write(&theme_path, "not valid xml").unwrap();

            let loaded = Theme::load_from_path(&theme_path);

            let default = Theme::default();
            assert_eq!(loaded.primary(), default.primary());
            assert_eq!(loaded.code_bg(), default.code_bg());
        }
    }
}
