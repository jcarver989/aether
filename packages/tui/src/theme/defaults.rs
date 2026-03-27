use super::{Color, Theme, darken_color, emphasize_color, lighten_color};

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

        Theme::builder()
            .fg(TEXT)
            .bg(BASE)
            .accent(ROSEWATER)
            .highlight_bg(Color::Rgb {
                r: 0x31,
                g: 0x4A,
                b: 0x56,
            })
            .highlight_fg(TEXT)
            .text_secondary(OVERLAY0)
            .code_fg(GREEN)
            .code_bg(SURFACE1)
            .heading(YELLOW)
            .link(BLUE)
            .blockquote(PINK)
            .muted(TEAL)
            .success(GREEN)
            .warning(PEACH)
            .error(RED)
            .info(BLUE)
            .secondary(MAUVE)
            .sidebar_bg(Color::Rgb {
                r: 0x24,
                g: 0x24,
                b: 0x36,
            })
            .diff_added_fg(GREEN)
            .diff_removed_fg(RED)
            .diff_added_bg(darken_color(GREEN))
            .diff_removed_bg(darken_color(RED))
            .diff_added_highlight_bg(emphasize_color(GREEN))
            .diff_removed_highlight_bg(emphasize_color(RED))
            .build()
            .expect("built-in catppuccin_mocha theme has all fields")
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

        Theme::builder()
            .fg(FG)
            .bg(BG)
            .accent(ACCENT)
            .highlight_bg(Color::Rgb {
                r: 0xDD,
                g: 0xDD,
                b: 0xDD,
            })
            .highlight_fg(FG)
            .text_secondary(Color::Rgb {
                r: 0x66,
                g: 0x66,
                b: 0x66,
            })
            .code_fg(Color::Rgb {
                r: 0x33,
                g: 0x66,
                b: 0x33,
            })
            .code_bg(Color::Rgb {
                r: 0xF0,
                g: 0xF0,
                b: 0xF0,
            })
            .heading(ACCENT)
            .link(Color::Rgb {
                r: 0x00,
                g: 0x44,
                b: 0xAA,
            })
            .blockquote(Color::Rgb {
                r: 0x66,
                g: 0x44,
                b: 0x88,
            })
            .muted(Color::Rgb {
                r: 0x88,
                g: 0x88,
                b: 0x88,
            })
            .success(GREEN)
            .warning(ORANGE)
            .error(RED)
            .info(ACCENT)
            .secondary(Color::Rgb {
                r: 0x66,
                g: 0x33,
                b: 0x99,
            })
            .sidebar_bg(Color::Rgb {
                r: 0xF4,
                g: 0xF4,
                b: 0xF8,
            })
            .diff_added_fg(GREEN)
            .diff_removed_fg(RED)
            .diff_added_bg(lighten_color(GREEN))
            .diff_removed_bg(lighten_color(RED))
            .diff_added_highlight_bg(lighten_color(GREEN))
            .diff_removed_highlight_bg(lighten_color(RED))
            .build()
            .expect("built-in light theme has all fields")
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::catppuccin_mocha()
    }
}
