use crate::settings::{WispSettings, resolve_theme_file_path};
use crossterm::style::Color;
use std::sync::{Arc, LazyLock};
use syntect::highlighting::{Highlighter, Theme as SyntectTheme, ThemeSet};
use syntect::parsing::Scope;
use tracing::warn;

/// Newtype over a `syntect` theme with semantic color accessors.
///
/// Cheap to clone (wraps an `Arc`). Derives markdown semantic colours from the
/// theme's `markup.*` scopes, falling back to theme-derived values when a scope
/// isn't defined. UI-status colours (success, warning, error, diff) stay fixed.
#[derive(Clone, Debug)]
pub struct Theme {
    syntect: Arc<SyntectTheme>,
    heading: Color,
    link: Color,
    blockquote: Color,
    muted: Color,
    code_fg: Color,
    text_secondary: Color,
}

static DEFAULT_THEME: LazyLock<Arc<SyntectTheme>> = LazyLock::new(|| {
    let cursor = std::io::Cursor::new(include_bytes!("../../assets/catppuccin-mocha.tmTheme"));
    let theme = ThemeSet::load_from_reader(&mut std::io::BufReader::new(cursor))
        .expect("embedded catppuccin-mocha.tmTheme is valid");
    Arc::new(theme)
});

impl Default for Theme {
    fn default() -> Self {
        Self::from_syntect(DEFAULT_THEME.clone())
    }
}

#[allow(dead_code, clippy::unused_self)]
impl Theme {
    pub fn primary(&self) -> Color {
        self.fg_color()
    }

    pub fn text_primary(&self) -> Color {
        self.fg_color()
    }

    pub fn code_fg(&self) -> Color {
        self.code_fg
    }

    pub fn code_bg(&self) -> Color {
        self.syntect
            .settings
            .background
            .map_or(DEFAULT_CODE_BG, color_from_syntect)
    }

    pub fn accent(&self) -> Color {
        self.syntect
            .settings
            .caret
            .map_or(DEFAULT_ACCENT, color_from_syntect)
    }

    pub fn highlight_bg(&self) -> Color {
        self.syntect
            .settings
            .selection
            .map_or(DEFAULT_HIGHLIGHT_BG, color_from_syntect)
    }

    pub fn secondary(&self) -> Color {
        SECONDARY
    }

    pub fn text_secondary(&self) -> Color {
        self.text_secondary
    }

    pub fn success(&self) -> Color {
        SUCCESS
    }

    pub fn warning(&self) -> Color {
        WARNING
    }

    pub fn error(&self) -> Color {
        ERROR
    }

    pub fn info(&self) -> Color {
        INFO
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
        DIFF_ADDED_BG
    }

    pub fn diff_removed_bg(&self) -> Color {
        DIFF_REMOVED_BG
    }

    pub fn diff_added_fg(&self) -> Color {
        DIFF_ADDED_FG
    }

    pub fn diff_removed_fg(&self) -> Color {
        DIFF_REMOVED_FG
    }

    pub fn syntect_theme(&self) -> &SyntectTheme {
        &self.syntect
    }

    pub fn load(settings: &WispSettings) -> Self {
        let Some(theme_file) = settings.theme.file.as_deref() else {
            return Self::default();
        };

        let Some(path) = resolve_theme_file_path(theme_file) else {
            warn!("Rejected unsafe theme filename: {}", theme_file);
            return Self::default();
        };

        match ThemeSet::get_theme(&path) {
            Ok(syntect_theme) => Self::from_syntect(Arc::new(syntect_theme)),
            Err(e) => {
                warn!(
                    "Failed to load theme from {}: {e}. Falling back to defaults.",
                    path.display()
                );
                Self::default()
            }
        }
    }

    fn from_syntect(syntect: Arc<SyntectTheme>) -> Self {
        let accent = syntect
            .settings
            .caret
            .map_or(DEFAULT_ACCENT, color_from_syntect);

        let text_secondary = derive_text_secondary(&syntect);

        let heading = resolve_scope_fg(&syntect, "markup.heading.markdown")
            .or_else(|| resolve_scope_fg(&syntect, "markup.heading"))
            .unwrap_or(accent);

        let link = resolve_scope_fg(&syntect, "markup.underline.link")
            .or_else(|| resolve_scope_fg(&syntect, "markup.link"))
            .unwrap_or(accent);

        let blockquote = resolve_scope_fg(&syntect, "markup.quote")
            .unwrap_or(text_secondary);

        let muted = resolve_scope_fg(&syntect, "markup.list.bullet")
            .or_else(|| {
                syntect
                    .settings
                    .gutter_foreground
                    .map(color_from_syntect)
            })
            .unwrap_or(text_secondary);

        let fg = syntect
            .settings
            .foreground
            .map_or(DEFAULT_FG, color_from_syntect);

        let code_fg = resolve_scope_fg(&syntect, "markup.inline.raw.string.markdown")
            .or_else(|| resolve_scope_fg(&syntect, "markup.raw"))
            .unwrap_or(fg);

        Self {
            syntect,
            heading,
            link,
            blockquote,
            muted,
            code_fg,
            text_secondary,
        }
    }

    fn fg_color(&self) -> Color {
        self.syntect
            .settings
            .foreground
            .map_or(DEFAULT_FG, color_from_syntect)
    }
}

/// Resolve the foreground color for a scope string against the theme.
/// Returns `None` if the scope doesn't parse, or if the resolved color matches
/// the theme's default foreground (meaning no specific rule matched).
fn resolve_scope_fg(theme: &SyntectTheme, scope_str: &str) -> Option<Color> {
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

/// Blend the theme's foreground toward its background at ~40% to produce a
/// naturally muted variant that adapts to both light and dark themes.
fn derive_text_secondary(theme: &SyntectTheme) -> Color {
    let fg = theme
        .settings
        .foreground
        .unwrap_or(syntect::highlighting::Color {
            r: 0xBF,
            g: 0xBD,
            b: 0xB6,
            a: 0xFF,
        });
    let bg = theme
        .settings
        .background
        .unwrap_or(syntect::highlighting::Color {
            r: 0x28,
            g: 0x28,
            b: 0x28,
            a: 0xFF,
        });

    #[allow(clippy::cast_possible_truncation)] // max = (255*60 + 255*40)/100 = 255
    let blend = |f: u8, b: u8| -> u8 {
        // 60% foreground + 40% background
        ((u16::from(f) * 60 + u16::from(b) * 40) / 100) as u8
    };

    Color::Rgb {
        r: blend(fg.r, bg.r),
        g: blend(fg.g, bg.g),
        b: blend(fg.b, bg.b),
    }
}

const DEFAULT_FG: Color = Color::Rgb {
    r: 0xBF,
    g: 0xBD,
    b: 0xB6,
};
const DEFAULT_CODE_BG: Color = Color::Rgb {
    r: 40,
    g: 40,
    b: 40,
};
const DEFAULT_ACCENT: Color = Color::Rgb {
    r: 255,
    g: 215,
    b: 0,
};
const DEFAULT_HIGHLIGHT_BG: Color = Color::Rgb {
    r: 0x1a,
    g: 0x4a,
    b: 0x50,
};

const SECONDARY: Color = Color::Rgb {
    r: 138,
    g: 43,
    b: 226,
};
const SUCCESS: Color = Color::Rgb {
    r: 0,
    g: 255,
    b: 127,
};
const WARNING: Color = Color::Rgb {
    r: 255,
    g: 165,
    b: 0,
};
const ERROR: Color = Color::Rgb {
    r: 255,
    g: 59,
    b: 48,
};
const INFO: Color = Color::Rgb {
    r: 78,
    g: 205,
    b: 196,
};

const DIFF_ADDED_BG: Color = Color::Rgb {
    r: 20,
    g: 50,
    b: 20,
};
const DIFF_REMOVED_BG: Color = Color::Rgb {
    r: 60,
    g: 20,
    b: 20,
};
const DIFF_ADDED_FG: Color = Color::Green;
const DIFF_REMOVED_FG: Color = Color::Red;

fn color_from_syntect(color: syntect::highlighting::Color) -> Color {
    Color::Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ThemeSettings as WispThemeSettings, WispSettings};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const CUSTOM_TMTHEME: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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
                <key>selection</key>
                <string>#334455</string>
            </dict>
        </dict>
    </array>
</dict>
</plist>"#;

    /// A theme that defines markup scopes with specific colors.
    const MARKUP_TMTHEME: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>name</key>
    <string>MarkupTest</string>
    <key>settings</key>
    <array>
        <dict>
            <key>settings</key>
            <dict>
                <key>foreground</key>
                <string>#AABBCC</string>
                <key>background</key>
                <string>#000000</string>
                <key>caret</key>
                <string>#FF0000</string>
            </dict>
        </dict>
        <dict>
            <key>name</key>
            <string>Heading</string>
            <key>scope</key>
            <string>markup.heading</string>
            <key>settings</key>
            <dict>
                <key>foreground</key>
                <string>#FF1111</string>
            </dict>
        </dict>
        <dict>
            <key>name</key>
            <string>Link</string>
            <key>scope</key>
            <string>markup.underline.link</string>
            <key>settings</key>
            <dict>
                <key>foreground</key>
                <string>#22FF22</string>
            </dict>
        </dict>
        <dict>
            <key>name</key>
            <string>Quote</string>
            <key>scope</key>
            <string>markup.quote</string>
            <key>settings</key>
            <dict>
                <key>foreground</key>
                <string>#3333FF</string>
            </dict>
        </dict>
        <dict>
            <key>name</key>
            <string>Raw</string>
            <key>scope</key>
            <string>markup.raw</string>
            <key>settings</key>
            <dict>
                <key>foreground</key>
                <string>#44FF44</string>
            </dict>
        </dict>
        <dict>
            <key>name</key>
            <string>Bullet</string>
            <key>scope</key>
            <string>markup.list.bullet</string>
            <key>settings</key>
            <dict>
                <key>foreground</key>
                <string>#FF5555</string>
            </dict>
        </dict>
    </array>
</dict>
</plist>"#;

    fn with_wisp_home(path: &Path, f: impl FnOnce()) {
        let old = std::env::var_os("WISP_HOME");
        // SAFETY: test-only; tests using this helper run serially (single-threaded test binary).
        unsafe { std::env::set_var("WISP_HOME", path) };
        f();
        if let Some(value) = old {
            unsafe { std::env::set_var("WISP_HOME", value) };
        } else {
            unsafe { std::env::remove_var("WISP_HOME") };
        }
    }

    #[test]
    fn default_theme_uses_embedded_catppuccin() {
        let theme = Theme::default();
        assert_eq!(
            theme.syntect_theme().name,
            Some("Catppuccin Mocha".to_string())
        );
    }

    #[test]
    fn default_theme_resolves_catppuccin_heading_color() {
        let theme = Theme::default();
        // Catppuccin mocha defines markup.heading as yellow (#f9e2af)
        assert_eq!(
            theme.heading(),
            Color::Rgb {
                r: 0xf9,
                g: 0xe2,
                b: 0xaf
            }
        );
    }

    #[test]
    fn default_theme_code_fg_differs_from_text_primary() {
        let theme = Theme::default();
        assert_ne!(
            theme.code_fg(),
            theme.text_primary(),
            "code_fg should be visually distinct from body text"
        );
    }

    #[test]
    fn custom_theme_with_markup_scopes_uses_those_colors() {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("markup.tmTheme"), MARKUP_TMTHEME).unwrap();

        let settings = WispSettings {
            theme: WispThemeSettings {
                file: Some("markup.tmTheme".to_string()),
            },
        };

        let loaded = {
            let mut result = Theme::default();
            with_wisp_home(temp_dir.path(), || {
                result = Theme::load(&settings);
            });
            result
        };

        assert_eq!(
            loaded.heading(),
            Color::Rgb {
                r: 0xFF,
                g: 0x11,
                b: 0x11
            }
        );
        assert_eq!(
            loaded.link(),
            Color::Rgb {
                r: 0x22,
                g: 0xFF,
                b: 0x22
            }
        );
        assert_eq!(
            loaded.blockquote(),
            Color::Rgb {
                r: 0x33,
                g: 0x33,
                b: 0xFF
            }
        );
        assert_eq!(
            loaded.code_fg(),
            Color::Rgb {
                r: 0x44,
                g: 0xFF,
                b: 0x44
            }
        );
        assert_eq!(
            loaded.muted(),
            Color::Rgb {
                r: 0xFF,
                g: 0x55,
                b: 0x55
            }
        );
    }

    #[test]
    fn sparse_theme_uses_theme_derived_fallbacks() {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        // CUSTOM_TMTHEME has no markup scopes, no caret — tests pure fallback path
        fs::write(themes_dir.join("custom.tmTheme"), CUSTOM_TMTHEME).unwrap();

        let settings = WispSettings {
            theme: WispThemeSettings {
                file: Some("custom.tmTheme".to_string()),
            },
        };

        let loaded = {
            let mut result = Theme::default();
            with_wisp_home(temp_dir.path(), || {
                result = Theme::load(&settings);
            });
            result
        };

        // heading/link should fall back to accent (DEFAULT_ACCENT since no caret)
        assert_eq!(loaded.heading(), DEFAULT_ACCENT);
        assert_eq!(loaded.link(), DEFAULT_ACCENT);

        // blockquote/muted should fall back to text_secondary (derived blend)
        let expected_secondary = loaded.text_secondary();
        assert_eq!(loaded.blockquote(), expected_secondary);

        // text_secondary should NOT be the old hardcoded constant
        assert_ne!(
            loaded.text_secondary(),
            Color::Rgb {
                r: 176,
                g: 176,
                b: 208
            },
            "text_secondary should be derived from theme, not hardcoded"
        );
    }

    #[test]
    fn valid_theme_file_loads_from_wisp_themes_dir() {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("custom.tmTheme"), CUSTOM_TMTHEME).unwrap();

        let settings = WispSettings {
            theme: WispThemeSettings {
                file: Some("custom.tmTheme".to_string()),
            },
        };

        let loaded = {
            let mut result = Theme::default();
            with_wisp_home(temp_dir.path(), || {
                result = Theme::load(&settings);
            });
            result
        };

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
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("broken.tmTheme"), "not valid xml").unwrap();

        let settings = WispSettings {
            theme: WispThemeSettings {
                file: Some("broken.tmTheme".to_string()),
            },
        };

        let loaded = {
            let mut result = Theme::default();
            with_wisp_home(temp_dir.path(), || {
                result = Theme::load(&settings);
            });
            result
        };

        let default = Theme::default();
        assert_eq!(loaded.primary(), default.primary());
        assert_eq!(loaded.code_bg(), default.code_bg());
    }

    #[test]
    fn path_traversal_rejected() {
        let settings = WispSettings {
            theme: WispThemeSettings {
                file: Some("../evil.tmTheme".to_string()),
            },
        };
        let loaded = Theme::load(&settings);
        let default = Theme::default();
        assert_eq!(loaded.primary(), default.primary());
    }
}
