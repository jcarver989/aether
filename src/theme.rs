use ratatui::style::Color;

// Catppuccin Mocha color palette
pub struct CatppuccinMocha;

impl CatppuccinMocha {
    // Neutral colors
    pub const CRUST: Color = Color::Rgb(17, 17, 27);
    pub const MANTLE: Color = Color::Rgb(24, 24, 37);
    pub const BASE: Color = Color::Rgb(30, 30, 46);
    pub const SURFACE_0: Color = Color::Rgb(49, 50, 68);
    pub const SURFACE_1: Color = Color::Rgb(69, 71, 90);
    pub const SURFACE_2: Color = Color::Rgb(88, 91, 112);
    pub const OVERLAY_0: Color = Color::Rgb(108, 112, 134);
    pub const OVERLAY_1: Color = Color::Rgb(127, 132, 156);
    pub const OVERLAY_2: Color = Color::Rgb(147, 153, 178);
    pub const SUBTEXT_0: Color = Color::Rgb(166, 173, 200);
    pub const SUBTEXT_1: Color = Color::Rgb(186, 194, 222);
    pub const TEXT: Color = Color::Rgb(205, 214, 244);
    
    // Accent colors
    pub const ROSEWATER: Color = Color::Rgb(245, 224, 220);
    pub const FLAMINGO: Color = Color::Rgb(242, 205, 205);
    pub const PINK: Color = Color::Rgb(245, 194, 231);
    pub const MAUVE: Color = Color::Rgb(203, 166, 247);
    pub const RED: Color = Color::Rgb(243, 139, 168);
    pub const MAROON: Color = Color::Rgb(235, 160, 172);
    pub const PEACH: Color = Color::Rgb(250, 179, 135);
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);
    pub const GREEN: Color = Color::Rgb(166, 227, 161);
    pub const TEAL: Color = Color::Rgb(148, 226, 213);
    pub const SKY: Color = Color::Rgb(137, 220, 235);
    pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236);
    pub const BLUE: Color = Color::Rgb(137, 180, 250);
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
}

#[derive(Clone, Debug)]
pub struct Theme {
    // Base colors
    pub background: Color,
    pub foreground: Color,
    pub muted: Color,
    pub subtle: Color,
    
    // Semantic colors
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
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
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub code_bg: Color,
    pub code_fg: Color,
    pub cursor_color: Color,
}

impl Default for Theme {
    fn default() -> Self {
        // Catppuccin Mocha theme using friendly names
        Self {
            // Base colors
            background: CatppuccinMocha::BASE,
            foreground: CatppuccinMocha::TEXT,
            muted: CatppuccinMocha::OVERLAY_0,
            subtle: CatppuccinMocha::SUBTEXT_0,
            
            // Semantic colors
            primary: CatppuccinMocha::BLUE,
            secondary: CatppuccinMocha::SAPPHIRE,
            accent: CatppuccinMocha::MAUVE,
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
            selection_bg: CatppuccinMocha::GREEN,
            selection_fg: CatppuccinMocha::CRUST,
            code_bg: CatppuccinMocha::SURFACE_0,
            code_fg: CatppuccinMocha::GREEN,
            cursor_color: CatppuccinMocha::OVERLAY_1,
        }
    }
}

impl Theme {
    pub fn dark() -> Self {
        Self::default()
    }
    
    pub fn light() -> Self {
        Self {
            background: Color::White,
            foreground: Color::Black,
            muted: Color::Gray,
            subtle: Color::DarkGray,
            
            primary: Color::Blue,
            secondary: Color::Cyan,
            accent: Color::Magenta,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            
            user_color: Color::Green,
            assistant_color: Color::Blue,
            system_color: Color::Magenta,
            tool_color: Color::Yellow,
            tool_call_color: Color::Cyan,
            tool_result_color: Color::Magenta,
            
            selection_bg: Color::Gray,
            selection_fg: Color::Black,
            code_bg: Color::Gray,
            code_fg: Color::Blue,
            cursor_color: Color::DarkGray,
        }
    }
    
    pub fn rgb() -> Self {
        // Catppuccin Mocha theme (same as default)
        Self::default()
    }
}