use crate::components::commands::TerminalCommand;

pub struct RenderContext {
    pub cursor_position: (u16, u16),
}

impl RenderContext {
    pub fn new(cursor_position: (u16, u16)) -> Self {
        Self { cursor_position }
    }
}

pub trait Component<T> {
    fn render(&self, props: T, context: &RenderContext) -> Vec<TerminalCommand>;
}
