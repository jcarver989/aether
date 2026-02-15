use crate::render_context::{Component, RenderContext};
use crate::screen::Line;
use crossterm::style::{Stylize, StyledContent};

pub struct InputPrompt;

impl Component for InputPrompt {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let styled: StyledContent<String> = "> ".to_string().with(context.theme.primary);
        vec![Line::new(format!("{styled}"))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_single_prompt_line() {
        let prompt = InputPrompt;
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert_eq!(lines.len(), 1);
        // The line contains "> " (with ANSI codes around it)
        assert!(lines[0].as_str().contains("> "));
    }

    #[test]
    fn renders_consistently() {
        let prompt = InputPrompt;
        let ctx = RenderContext::new((80, 24));
        let a = prompt.render(&ctx);
        let b = prompt.render(&ctx);
        assert_eq!(a, b);
    }
}
