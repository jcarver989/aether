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

pub struct Container<'a> {
    children: Vec<&'a dyn Component>,
}

impl<'a> Container<'a> {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    pub fn add(&mut self, component: &'a dyn Component) -> &mut Self {
        self.children.push(component);
        self
    }
}

impl Component for Container<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        self.children
            .iter()
            .flat_map(|c| c.render(context))
            .collect()
    }
}
