use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub system: Color,
    pub user: Color,
    pub assistant: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            system: Color::Magenta,
            user: Color::Green,
            assistant: Color::Blue,
        }
    }
}
