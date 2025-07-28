use ratatui::style::Color;

// Catppuccin Mocha color palette
pub struct CatppuccinMocha;

impl CatppuccinMocha {
    // Neutral colors
    pub const CRUST: Color = Color::Rgb(17, 17, 27);
    pub const BASE: Color = Color::Rgb(30, 30, 46);
    pub const SURFACE_0: Color = Color::Rgb(49, 50, 68);
    pub const OVERLAY_0: Color = Color::Rgb(108, 112, 134);
    pub const OVERLAY_1: Color = Color::Rgb(127, 132, 156);
    pub const SUBTEXT_0: Color = Color::Rgb(166, 173, 200);
    pub const TEXT: Color = Color::Rgb(205, 214, 244);

    // Accent colors
    pub const MAUVE: Color = Color::Rgb(203, 166, 247);
    pub const RED: Color = Color::Rgb(243, 139, 168);
    pub const PEACH: Color = Color::Rgb(250, 179, 135);
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);
    pub const GREEN: Color = Color::Rgb(166, 227, 161);
    pub const TEAL: Color = Color::Rgb(148, 226, 213);
    pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236);
    pub const BLUE: Color = Color::Rgb(137, 180, 250);
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
}

#[derive(Clone, Debug)]
pub struct Theme {
    // Base colors
    pub foreground: Color,
    pub muted: Color,
    pub subtle: Color,

    // Semantic colors
    pub success: Color,
    pub warning: Color,
    pub error: Color,

    // Role-specific colors
    pub user_color: Color,
    pub assistant_color: Color,
    pub system_color: Color,
    pub tool_color: Color,
    pub tool_call_color: Color,
    pub tool_result_color: Color,

    // UI element colors
    pub code_bg: Color,
    pub code_fg: Color,
    pub cursor_color: Color,
}

impl Default for Theme {
    fn default() -> Self {
        // Catppuccin Mocha theme using friendly names
        Self {
            // Base colors
            foreground: CatppuccinMocha::TEXT,
            muted: CatppuccinMocha::OVERLAY_0,
            subtle: CatppuccinMocha::SUBTEXT_0,

            // Semantic colors
            success: CatppuccinMocha::GREEN,
            warning: CatppuccinMocha::YELLOW,
            error: CatppuccinMocha::RED,

            // Role-specific colors
            user_color: CatppuccinMocha::GREEN,
            assistant_color: CatppuccinMocha::BLUE,
            system_color: CatppuccinMocha::LAVENDER,
            tool_color: CatppuccinMocha::PEACH,
            tool_call_color: CatppuccinMocha::TEAL,
            tool_result_color: CatppuccinMocha::MAUVE,

            // UI element colors
            code_bg: CatppuccinMocha::SURFACE_0,
            code_fg: CatppuccinMocha::GREEN,
            cursor_color: CatppuccinMocha::OVERLAY_1,
        }
    }
}
