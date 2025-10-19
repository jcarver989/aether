use crate::colors::Theme;
use crate::components::commands::TerminalCommand;

pub struct RenderContext {
    pub cursor_position: (u16, u16),
    pub size: (u16, u16),
    pub theme: Theme,
}

impl RenderContext {
    pub fn new(cursor_position: (u16, u16), size: (u16, u16)) -> Self {
        Self {
            cursor_position,
            size,
            theme: Theme::default(),
        }
    }
}

pub trait Component<T> {
    fn render(&self, props: T, context: &RenderContext) -> Vec<TerminalCommand>;
}
