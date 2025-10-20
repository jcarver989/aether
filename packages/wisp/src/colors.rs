use crossterm::style::Color;

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
        }
    }
}
