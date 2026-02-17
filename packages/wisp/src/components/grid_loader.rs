use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;

pub(crate) const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

#[derive(Default)]
pub struct GridLoader {
    pub tick: u16,
    pub visible: bool,
}

impl GridLoader {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for GridLoader {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        if !self.visible {
            return vec![];
        }

        let frame = FRAMES[self.tick as usize % FRAMES.len()];
        let styled = format!("{}", frame.with(context.theme.info));
        vec![Line::new(format!("  {styled}"))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invisible_renders_empty() {
        let loader = GridLoader {
            tick: 0,
            visible: false,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = loader.render(&ctx);
        assert!(lines.is_empty());
    }

    #[test]
    fn visible_renders_one_line() {
        let loader = GridLoader {
            tick: 0,
            visible: true,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = loader.render(&ctx);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn different_ticks_produce_different_output() {
        let ctx = RenderContext::new((80, 24));

        let loader_a = GridLoader {
            tick: 0,
            visible: true,
        };
        let loader_b = GridLoader {
            tick: 1,
            visible: true,
        };

        let a = loader_a.render(&ctx)[0].as_str().to_string();
        let b = loader_b.render(&ctx)[0].as_str().to_string();

        assert_ne!(a, b);
    }

    #[test]
    fn cycles_after_full_rotation() {
        let ctx = RenderContext::new((80, 24));

        let loader_a = GridLoader {
            tick: 0,
            visible: true,
        };
        let loader_b = GridLoader {
            tick: FRAMES.len() as u16,
            visible: true,
        };

        let a = loader_a.render(&ctx)[0].as_str().to_string();
        let b = loader_b.render(&ctx)[0].as_str().to_string();

        assert_eq!(a, b);
    }
}
