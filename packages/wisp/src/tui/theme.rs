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
    success: Color,
    warning: Color,
    error: Color,
    info: Color,
    secondary: Color,
    diff_added_fg: Color,
    diff_removed_fg: Color,
    diff_added_bg: Color,
    diff_removed_bg: Color,
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

    /// Returns the background color for a mode badge, cycling through a
    /// fixed palette based on the mode's index in the options list.
    pub fn mode_badge_bg(&self, index: usize) -> Color {
        const PALETTE_FIELDS: [fn(&Theme) -> Color; 3] = [
            |t| t.info,
            |t| t.secondary,
            |t| t.warning,
        ];
        PALETTE_FIELDS[index % PALETTE_FIELDS.len()](self)
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

    #[allow(clippy::similar_names)] // diff_added_fg/bg and diff_removed_fg/bg are intentionally paired
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

        let blockquote = resolve_scope_fg(&syntect, "markup.quote").unwrap_or(text_secondary);

        let muted = resolve_scope_fg(&syntect, "markup.list.bullet")
            .or_else(|| syntect.settings.gutter_foreground.map(color_from_syntect))
            .unwrap_or(text_secondary);

        let fg = syntect
            .settings
            .foreground
            .map_or(DEFAULT_FG, color_from_syntect);

        let code_fg = resolve_scope_fg(&syntect, "markup.inline.raw.string.markdown")
            .or_else(|| resolve_scope_fg(&syntect, "markup.raw"))
            .unwrap_or(fg);

        let error = resolve_scope_fg(&syntect, "markup.deleted")
            .or_else(|| resolve_scope_fg(&syntect, "markup.deleted.diff"))
            .or_else(|| resolve_scope_fg(&syntect, "invalid"))
            .unwrap_or(accent);

        let warning = resolve_scope_fg(&syntect, "constant.numeric").unwrap_or(accent);

        let success = resolve_scope_fg(&syntect, "markup.inserted")
            .or_else(|| resolve_scope_fg(&syntect, "markup.inserted.diff"))
            .or_else(|| resolve_scope_fg(&syntect, "string"))
            .unwrap_or(accent);

        let info = resolve_scope_fg(&syntect, "entity.name.function")
            .or_else(|| resolve_scope_fg(&syntect, "support.function"))
            .unwrap_or(accent);

        let secondary = resolve_scope_fg(&syntect, "keyword")
            .or_else(|| resolve_scope_fg(&syntect, "storage.type"))
            .unwrap_or(accent);

        let diff_added_fg = resolve_scope_fg(&syntect, "markup.inserted.diff")
            .or_else(|| resolve_scope_fg(&syntect, "markup.inserted"))
            .or_else(|| resolve_scope_fg(&syntect, "string"))
            .unwrap_or(accent);

        let diff_removed_fg = resolve_scope_fg(&syntect, "markup.deleted.diff")
            .or_else(|| resolve_scope_fg(&syntect, "markup.deleted"))
            .unwrap_or(accent);

        let diff_added_bg = darken_color(diff_added_fg);
        let diff_removed_bg = darken_color(diff_removed_fg);

        Self {
            syntect,
            heading,
            link,
            blockquote,
            muted,
            code_fg,
            text_secondary,
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

/// Darken a color to ~20% brightness for use as a subtle background.
#[allow(clippy::cast_possible_truncation)] // max input is 255, 255*20/100 = 51
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
    use syntect::highlighting::ThemeSettings;
    use tempfile::TempDir;

    /// Catppuccin Mocha palette constants used by the embedded theme.
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
    }; // teal (markup.list.bullet)
    const CATPPUCCIN_CODE_FG: Color = Color::Rgb {
        r: 0xa6,
        g: 0xe3,
        b: 0xa1,
    }; // green (markup.inline.raw)
    const CATPPUCCIN_ERROR: Color = Color::Rgb {
        r: 0xf3,
        g: 0x8b,
        b: 0xa8,
    }; // red (markup.deleted)
    const CATPPUCCIN_WARNING: Color = Color::Rgb {
        r: 0xfa,
        g: 0xb3,
        b: 0x87,
    }; // peach (constant.numeric)
    const CATPPUCCIN_SUCCESS: Color = Color::Rgb {
        r: 0xa6,
        g: 0xe3,
        b: 0xa1,
    }; // green (markup.inserted)
    const CATPPUCCIN_INFO: Color = Color::Rgb {
        r: 0x89,
        g: 0xb4,
        b: 0xfa,
    }; // blue (entity.name.function)
    const CATPPUCCIN_SECONDARY: Color = Color::Rgb {
        r: 0xcb,
        g: 0xa6,
        b: 0xf7,
    }; // mauve (keyword)
    const CATPPUCCIN_ACCENT: Color = Color::Rgb {
        r: 0xf5,
        g: 0xe0,
        b: 0xdc,
    }; // rosewater (caret)

    /// Build a bare `SyntectTheme` with only global settings, no scope rules.
    /// Used to test that all colors fall back to accent.
    fn bare_syntect_theme() -> Arc<SyntectTheme> {
        Arc::new(SyntectTheme {
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

    /// Build a bare theme with no caret, so accent falls back to `DEFAULT_ACCENT`.
    fn bare_syntect_theme_no_caret() -> Arc<SyntectTheme> {
        Arc::new(SyntectTheme {
            name: Some("BareNoCaret".into()),
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
                ..ThemeSettings::default()
            },
            scopes: Vec::new(),
        })
    }

    /// Inline XML theme for file-loading tests only (Theme::load needs a file).
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

    /// Mutex that serializes all tests which mutate the `WISP_HOME` env var.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_wisp_home(path: &Path, f: impl FnOnce()) {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("WISP_HOME");
        // SAFETY: test-only; serialized by ENV_MUTEX above.
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
    fn catppuccin_resolves_markdown_colors() {
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
        // diff fg comes from markup.inserted.diff / markup.deleted.diff
        assert_eq!(theme.diff_added_fg(), CATPPUCCIN_SUCCESS);
        assert_eq!(theme.diff_removed_fg(), CATPPUCCIN_ERROR);
        // diff bg is darkened to ~20%
        assert_eq!(theme.diff_added_bg(), darken_color(CATPPUCCIN_SUCCESS));
        assert_eq!(theme.diff_removed_bg(), darken_color(CATPPUCCIN_ERROR));
    }

    #[test]
    fn catppuccin_accent_is_caret_color() {
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
    fn bare_theme_falls_back_to_accent() {
        let accent = Color::Rgb {
            r: 0xAA,
            g: 0xBB,
            b: 0xCC,
        };
        let theme = Theme::from_syntect(bare_syntect_theme());

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
    fn bare_theme_no_caret_falls_back_to_default_accent() {
        let theme = Theme::from_syntect(bare_syntect_theme_no_caret());

        assert_eq!(theme.heading(), DEFAULT_ACCENT);
        assert_eq!(theme.link(), DEFAULT_ACCENT);
        assert_eq!(theme.error(), DEFAULT_ACCENT);
        assert_eq!(theme.warning(), DEFAULT_ACCENT);
        assert_eq!(theme.success(), DEFAULT_ACCENT);
        assert_eq!(theme.info(), DEFAULT_ACCENT);
        assert_eq!(theme.secondary(), DEFAULT_ACCENT);
    }

    #[test]
    fn bare_theme_blockquote_falls_back_to_text_secondary() {
        let theme = Theme::from_syntect(bare_syntect_theme());
        assert_eq!(theme.blockquote(), theme.text_secondary());
    }

    #[test]
    fn bare_theme_diff_bg_is_darkened_accent() {
        let accent = Color::Rgb {
            r: 0xAA,
            g: 0xBB,
            b: 0xCC,
        };
        let theme = Theme::from_syntect(bare_syntect_theme());
        assert_eq!(theme.diff_added_bg(), darken_color(accent));
        assert_eq!(theme.diff_removed_bg(), darken_color(accent));
    }

    #[test]
    fn valid_theme_file_loads_from_wisp_themes_dir() {
        let temp_dir = TempDir::new().unwrap();
        let themes_dir = temp_dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("custom.tmTheme"), LOADABLE_TMTHEME).unwrap();

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

    #[test]
    fn mode_badge_bg_cycles_through_palette() {
        let theme = Theme::default();
        assert_eq!(theme.mode_badge_bg(0), CATPPUCCIN_INFO);
        assert_eq!(theme.mode_badge_bg(1), CATPPUCCIN_SECONDARY);
        assert_eq!(theme.mode_badge_bg(2), CATPPUCCIN_WARNING);
        // Wraps around
        assert_eq!(theme.mode_badge_bg(3), CATPPUCCIN_INFO);
    }

    #[test]
    fn mode_badge_bg_distinct_for_adjacent_indices() {
        let theme = Theme::default();
        assert_ne!(
            theme.mode_badge_bg(0),
            theme.mode_badge_bg(1),
            "adjacent indices should have distinct badge colors"
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
}
