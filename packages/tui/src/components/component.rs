use crate::line::Line;
pub use crate::rendering::render_context::RenderContext;

/// A stateful widget that can render itself as styled terminal lines.
pub trait Component {
    fn render(&self, context: &RenderContext) -> Vec<Line>;
}
