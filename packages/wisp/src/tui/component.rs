use super::screen::Line;
use super::theme::Theme;

pub struct RenderContext {
    #[allow(dead_code)]
    pub size: (u16, u16),
    pub theme: Theme,
}

impl RenderContext {
    pub fn new(size: (u16, u16)) -> Self {
        Self {
            size,
            theme: Theme::default(),
        }
    }
}

pub trait Component {
    fn render(&self, context: &RenderContext) -> Vec<Line>;
}
