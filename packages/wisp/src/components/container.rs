use crate::tui::{Component, Line, RenderContext};

pub struct Container<'a> {
    children: Vec<&'a dyn Component>,
}

impl<'a> Container<'a> {
    pub fn new(children: Vec<&'a dyn Component>) -> Self {
        Self { children }
    }

    pub fn push(&mut self, child: &'a dyn Component) {
        self.children.push(child);
    }

    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn render_with_offsets(&self, context: &RenderContext) -> (Vec<Line>, Vec<usize>) {
        let mut lines = Vec::new();
        let mut offsets = Vec::with_capacity(self.children.len());

        for child in &self.children {
            offsets.push(lines.len());
            lines.extend(child.render(context));
        }

        (lines, offsets)
    }
}

impl Component for Container<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        self.render_with_offsets(context).0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubComponent {
        lines: Vec<Line>,
    }

    impl Component for StubComponent {
        fn render(&self, _context: &RenderContext) -> Vec<Line> {
            self.lines.clone()
        }
    }

    #[test]
    fn renders_empty_container() {
        let container = Container::new(Vec::new());
        let context = RenderContext::new((80, 24));
        let lines = container.render(&context);
        assert!(lines.is_empty());
    }

    #[test]
    fn preserves_child_order() {
        let a = StubComponent {
            lines: vec![Line::new("a")],
        };
        let b = StubComponent {
            lines: vec![Line::new("b")],
        };
        let container = Container::new(vec![&a, &b]);
        let context = RenderContext::new((80, 24));
        let lines = container.render(&context);
        assert_eq!(lines, vec![Line::new("a"), Line::new("b")]);
    }

    #[test]
    fn computes_offsets_per_child() {
        let a = StubComponent {
            lines: vec![Line::new("a1"), Line::new("a2")],
        };
        let b = StubComponent {
            lines: vec![Line::new("b1")],
        };
        let container = Container::new(vec![&a, &b]);
        let context = RenderContext::new((80, 24));
        let (_lines, offsets) = container.render_with_offsets(&context);
        assert_eq!(offsets, vec![0, 2]);
    }
}
