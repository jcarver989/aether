use ratatui::style::Color;

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
        // 256-color theme optimized for xterm-256 terminals
        Self {
            // Base colors - sophisticated grays
            background: Color::Indexed(0),     // Black
            foreground: Color::Indexed(15),    // Bright white
            muted: Color::Indexed(240),        // Dark gray
            subtle: Color::Indexed(244),       // Medium gray
            
            // Semantic colors - vibrant but not harsh
            primary: Color::Indexed(39),       // Bright blue
            secondary: Color::Indexed(51),     // Bright cyan  
            accent: Color::Indexed(201),       // Bright magenta
            success: Color::Indexed(46),       // Bright green
            warning: Color::Indexed(220),      // Golden yellow
            error: Color::Indexed(196),        // Bright red
            
            // Role-specific colors - distinct and harmonious
            user_color: Color::Indexed(82),     // Light green
            assistant_color: Color::Indexed(75), // Light blue
            system_color: Color::Indexed(141),  // Light purple
            tool_color: Color::Indexed(214),    // Orange
            tool_call_color: Color::Indexed(87), // Teal
            tool_result_color: Color::Indexed(177), // Lavender
            
            // UI element colors - subtle but clear
            selection_bg: Color::Indexed(236),  // Very dark gray
            selection_fg: Color::Indexed(255), // Pure white
            code_bg: Color::Indexed(234),      // Darker gray
            code_fg: Color::Indexed(120),      // Light green
            cursor_color: Color::Indexed(242), // Medium gray
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
        // RGB version for terminals that support true color
        Self {
            // Base colors - modern gray scale
            background: Color::Rgb(17, 24, 39),     // gray-900
            foreground: Color::Rgb(248, 249, 250),  // gray-50
            muted: Color::Rgb(108, 117, 125),       // gray-600
            subtle: Color::Rgb(156, 163, 175),      // gray-400
            
            // Semantic colors - Tailwind-inspired
            primary: Color::Rgb(59, 130, 246),      // blue-500
            secondary: Color::Rgb(6, 182, 212),     // cyan-500
            accent: Color::Rgb(168, 85, 247),       // purple-500
            success: Color::Rgb(34, 197, 94),       // green-500
            warning: Color::Rgb(251, 191, 36),      // amber-400
            error: Color::Rgb(239, 68, 68),         // red-500
            
            // Role-specific colors
            user_color: Color::Rgb(72, 187, 120),   // emerald-400
            assistant_color: Color::Rgb(59, 130, 246), // blue-500
            system_color: Color::Rgb(147, 112, 219), // mediumpurple
            tool_color: Color::Rgb(251, 191, 36),   // amber-400
            tool_call_color: Color::Rgb(6, 182, 212), // cyan-500
            tool_result_color: Color::Rgb(168, 85, 247), // purple-500
            
            // UI element colors
            selection_bg: Color::Rgb(55, 65, 81),   // gray-700
            selection_fg: Color::Rgb(248, 249, 250), // gray-50
            code_bg: Color::Rgb(31, 41, 55),        // gray-800
            code_fg: Color::Rgb(34, 197, 94),       // green-500
            cursor_color: Color::Rgb(156, 163, 175), // gray-400
        }
    }
}