use crossterm::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffTheme {
    pub added_bg: Color,
    pub removed_bg: Color,
    pub added_fg: Color,
    pub removed_fg: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub muted: Color,
    pub heading: Color,
    pub code_fg: Color,
    pub code_bg: Color,
    pub diff: DiffTheme,
    pub link: Color,
    pub blockquote: Color,
    pub highlight_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            secondary: Color::Rgb {
                r: 138,
                g: 43,
                b: 226,
            }, // Electric Violet #8A2BE2
            primary: Color::Rgb {
                r: 0x55,
                g: 0xc9,
                b: 0xd3,
            }, // #55c9d3
            accent: Color::Rgb {
                r: 255,
                g: 215,
                b: 0,
            }, // Gold #FFD700
            text_primary: Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            }, // White #FFFFFF
            text_secondary: Color::Rgb {
                r: 176,
                g: 176,
                b: 208,
            }, // Light Gray #B0B0D0
            success: Color::Rgb {
                r: 0,
                g: 255,
                b: 127,
            }, // Spring Green #00FF7F
            warning: Color::Rgb {
                r: 255,
                g: 165,
                b: 0,
            }, // Orange #FFA500
            error: Color::Rgb {
                r: 255,
                g: 59,
                b: 48,
            }, // Red #FF3B30
            info: Color::Rgb {
                r: 78,
                g: 205,
                b: 196,
            }, // Turquoise #4ECDC4
            muted: Color::Rgb {
                r: 128,
                g: 128,
                b: 128,
            }, // Gray #808080
            heading: Color::Rgb {
                r: 0x55,
                g: 0xc9,
                b: 0xd3,
            }, // Same as primary #55c9d3
            code_fg: Color::Rgb {
                r: 200,
                g: 200,
                b: 200,
            }, // Light gray #C8C8C8
            code_bg: Color::Rgb {
                r: 40,
                g: 40,
                b: 40,
            }, // Dark gray #282828
            diff: DiffTheme {
                added_bg: Color::Rgb {
                    r: 20,
                    g: 50,
                    b: 20,
                },
                removed_bg: Color::Rgb {
                    r: 60,
                    g: 20,
                    b: 20,
                },
                added_fg: Color::Green,
                removed_fg: Color::Red,
            },
            link: Color::Rgb {
                r: 78,
                g: 205,
                b: 196,
            }, // Same as info #4ECDC4
            blockquote: Color::Rgb {
                r: 128,
                g: 128,
                b: 128,
            }, // Same as muted #808080
            highlight_bg: Color::Rgb {
                r: 0x1a,
                g: 0x4a,
                b: 0x50,
            }, // Dark teal #1A4A50
        }
    }
}
