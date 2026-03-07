use crate::tui::{Component, Line, RenderContext, Style};
use crossterm::style::Color;
use unicode_width::UnicodeWidthStr;

/// Width consumed by left ("│ ") and right (" │") borders.
const BORDER_H_PAD: usize = 4;

pub struct Container<'a> {
    children: Vec<&'a mut dyn Component>,
    title: Option<String>,
    footer: Option<String>,
    border_color: Option<Color>,
    fill_height: Option<usize>,
    gap: usize,
}

impl<'a> Container<'a> {
    pub fn new(children: Vec<&'a mut dyn Component>) -> Self {
        Self {
            children,
            title: None,
            footer: None,
            border_color: None,
            fill_height: None,
            gap: 0,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = Some(color);
        self
    }

    pub fn fill_height(mut self, h: usize) -> Self {
        self.fill_height = Some(h);
        self
    }

    pub fn gap(mut self, lines: usize) -> Self {
        self.gap = lines;
        self
    }

    pub fn push(&mut self, child: &'a mut dyn Component) {
        self.children.push(child);
    }

    pub fn len(&self) -> usize {
        self.children.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    pub fn render_with_offsets(&mut self, context: &RenderContext) -> (Vec<Line>, Vec<usize>) {
        let mut lines = Vec::new();
        let mut offsets = Vec::with_capacity(self.children.len());

        for (i, child) in self.children.iter_mut().enumerate() {
            if i > 0 {
                for _ in 0..self.gap {
                    lines.push(Line::default());
                }
            }
            offsets.push(lines.len());
            lines.extend(child.render(context));
        }

        (lines, offsets)
    }
}

impl Component for Container<'_> {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
        let has_border =
            self.border_color.is_some() || self.title.is_some() || self.footer.is_some();

        if !has_border {
            return self.render_with_offsets(context).0;
        }

        let width = context.size.0 as usize;
        let inner_width = width.saturating_sub(BORDER_H_PAD);
        let border_style = self.border_color.map(Style::fg).unwrap_or_default();

        // Compute chrome overhead so we can tell children how much space they have.
        // Chrome: top border (1) + blank line (1) + bottom border (1) + optional footer (1).
        let chrome_lines = 3 + usize::from(self.footer.is_some());
        let gap_lines = if self.children.len() > 1 {
            self.gap * (self.children.len() - 1)
        } else {
            0
        };
        let total_for_children = self
            .fill_height
            .map(|fh| fh.saturating_sub(chrome_lines + gap_lines));

        #[allow(clippy::cast_possible_truncation)] // inner_width ≤ context.size.0 (u16)
        let inner_context = context.with_size((inner_width as u16, context.size.1));

        let mut lines = Vec::new();

        // ── Top border ──
        let title_text = self.title.as_deref().unwrap_or("");
        let bar_left = "┌─";
        let bar_right_pad = width.saturating_sub(
            UnicodeWidthStr::width(bar_left) + UnicodeWidthStr::width(title_text) + 1,
        ); // 1 for ┐
        let title_line = format!(
            "{bar_left}{title_text}{:─>bar_right_pad$}┐",
            "",
            bar_right_pad = bar_right_pad
        );
        lines.push(Line::with_style(title_line, border_style));

        // ── Blank line after top border ──
        lines.push(empty_border_line(inner_width));

        // ── Render children with gap ──
        // Track how many content rows have been consumed so far, so that
        // later children receive a `max_height` reflecting remaining space.
        let mut content_rows_used: usize = 0;

        for (i, child) in self.children.iter_mut().enumerate() {
            if i > 0 {
                for _ in 0..self.gap {
                    lines.push(empty_border_line(inner_width));
                }
                content_rows_used += self.gap;
            }

            // For children after the first, pass remaining rows via max_height.
            let child_ctx = if i > 0 {
                if let Some(total) = total_for_children {
                    inner_context.with_max_height(total.saturating_sub(content_rows_used))
                } else {
                    inner_context.with_max_height(usize::MAX)
                }
            } else {
                inner_context.without_max_height()
            };

            let child_lines = child.render(&child_ctx);
            content_rows_used += child_lines.len();
            for cl in &child_lines {
                lines.push(wrap_in_border(cl, inner_width));
            }
        }

        // ── Fill padding ──
        if let Some(target_height) = self.fill_height {
            // Reserve space for footer (1) + bottom border (1) = 2
            let reserved = if self.footer.is_some() { 2 } else { 1 };
            let target_content = target_height.saturating_sub(reserved);
            while lines.len() < target_content {
                lines.push(empty_border_line(inner_width));
            }
        }

        // ── Footer ──
        if let Some(ref footer_text) = self.footer {
            let footer_pad =
                inner_width.saturating_sub(UnicodeWidthStr::width(footer_text.as_str()));
            let footer_line_str = format!(
                "│ {footer_text}{:footer_pad$} │",
                "",
                footer_pad = footer_pad
            );
            lines.push(Line::with_style(footer_line_str, border_style));
        }

        // ── Bottom border ──
        let bottom_inner = width.saturating_sub(2); // └ and ┘
        let bottom_line = format!("└{:─>bottom_inner$}┘", "", bottom_inner = bottom_inner);
        lines.push(Line::with_style(bottom_line, border_style));

        // Clamp to fill_height if set
        if let Some(target_height) = self.fill_height {
            lines.truncate(target_height);
        }

        lines
    }
}

/// Wrap a content line with `│ ... │` borders, extending any bg color through
/// the padding so the highlight fills the full row width.
fn wrap_in_border(content: &Line, inner_width: usize) -> Line {
    let mut padded_content = content.clone();
    padded_content.extend_bg_to_width(inner_width);

    let mut line = Line::new("│ ".to_string());
    line.append_line(&padded_content);
    line.push_text(" │".to_string());
    line
}

fn empty_border_line(inner_width: usize) -> Line {
    Line::new(format!(
        "│ {:inner_width$} │",
        "",
        inner_width = inner_width
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubComponent {
        lines: Vec<Line>,
    }

    impl Component for StubComponent {
        fn render(&mut self, _context: &RenderContext) -> Vec<Line> {
            self.lines.clone()
        }
    }

    #[test]
    fn renders_empty_container() {
        let mut container = Container::new(Vec::new());
        let context = RenderContext::new((80, 24));
        let lines = container.render(&context);
        assert!(lines.is_empty());
    }

    #[test]
    fn preserves_child_order() {
        let mut a = StubComponent {
            lines: vec![Line::new("a")],
        };
        let mut b = StubComponent {
            lines: vec![Line::new("b")],
        };
        let mut container = Container::new(vec![&mut a, &mut b]);
        let context = RenderContext::new((80, 24));
        let lines = container.render(&context);
        assert_eq!(lines, vec![Line::new("a"), Line::new("b")]);
    }

    #[test]
    fn computes_offsets_per_child() {
        let mut a = StubComponent {
            lines: vec![Line::new("a1"), Line::new("a2")],
        };
        let mut b = StubComponent {
            lines: vec![Line::new("b1")],
        };
        let mut container = Container::new(vec![&mut a, &mut b]);
        let context = RenderContext::new((80, 24));
        let (_lines, offsets) = container.render_with_offsets(&context);
        assert_eq!(offsets, vec![0, 2]);
    }

    #[test]
    fn gap_inserts_blank_lines_between_children() {
        let mut a = StubComponent {
            lines: vec![Line::new("a")],
        };
        let mut b = StubComponent {
            lines: vec![Line::new("b")],
        };
        let mut container = Container::new(vec![&mut a, &mut b]).gap(1);
        let context = RenderContext::new((80, 24));
        let lines = container.render(&context);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], Line::new("a"));
        assert!(lines[1].plain_text().is_empty());
        assert_eq!(lines[2], Line::new("b"));
    }

    #[test]
    fn gap_no_blank_line_before_first_child() {
        let mut a = StubComponent {
            lines: vec![Line::new("a")],
        };
        let mut container = Container::new(vec![&mut a]).gap(2);
        let context = RenderContext::new((80, 24));
        let lines = container.render(&context);
        // Single child: no gap lines
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], Line::new("a"));
    }

    #[test]
    fn title_renders_top_border_with_title_text() {
        let mut a = StubComponent {
            lines: vec![Line::new("x")],
        };
        let mut container = Container::new(vec![&mut a])
            .title(" Config ")
            .border_color(Color::Grey);
        let context = RenderContext::new((30, 10));
        let lines = container.render(&context);
        let top = lines[0].plain_text();
        assert!(top.starts_with("┌─ Config "), "top: {top}");
        assert!(top.ends_with('┐'), "top: {top}");
    }

    #[test]
    fn footer_renders_footer_and_bottom_border() {
        let mut a = StubComponent {
            lines: vec![Line::new("x")],
        };
        let mut container = Container::new(vec![&mut a])
            .footer("[Esc] Close")
            .border_color(Color::Grey);
        let context = RenderContext::new((30, 10));
        let lines = container.render(&context);
        let last = lines.last().unwrap().plain_text();
        assert!(last.starts_with('└'), "last: {last}");
        assert!(last.ends_with('┘'), "last: {last}");
        let footer = lines[lines.len() - 2].plain_text();
        assert!(footer.contains("[Esc] Close"), "footer: {footer}");
    }

    #[test]
    fn fill_height_pads_with_empty_bordered_rows() {
        let mut a = StubComponent {
            lines: vec![Line::new("x")],
        };
        let mut container = Container::new(vec![&mut a])
            .title(" T ")
            .footer("F")
            .border_color(Color::Grey)
            .fill_height(10);
        let context = RenderContext::new((30, 10));
        let lines = container.render(&context);
        assert_eq!(lines.len(), 10, "should fill to exactly 10 lines");
    }

    #[test]
    fn border_color_styles_border_lines() {
        let mut a = StubComponent {
            lines: vec![Line::new("x")],
        };
        let mut container = Container::new(vec![&mut a])
            .title(" T ")
            .border_color(Color::Cyan);
        let context = RenderContext::new((30, 10));
        let lines = container.render(&context);
        // Top border should have Cyan fg
        let top_span = &lines[0].spans()[0];
        assert_eq!(top_span.style().fg, Some(Color::Cyan));
        // Bottom border should have Cyan fg
        let bottom_span = &lines.last().unwrap().spans()[0];
        assert_eq!(bottom_span.style().fg, Some(Color::Cyan));
    }

    #[test]
    fn bg_color_extends_through_padding() {
        let bg = Color::DarkBlue;
        let mut a = StubComponent {
            lines: vec![Line::with_style("hi", Style::default().bg_color(bg))],
        };
        let mut container = Container::new(vec![&mut a]).border_color(Color::Grey);
        let context = RenderContext::new((20, 10));
        let lines = container.render(&context);
        // Content row (top border + blank + first content = index 2)
        let content_row = &lines[2];
        // The bg-colored span should cover "hi" + padding, stretching across
        // inner_width. Find any span with the bg color and check its text
        // extends beyond just "hi".
        let bg_span = content_row
            .spans()
            .iter()
            .find(|s| s.style().bg == Some(bg))
            .expect("should have a span with bg color");
        assert!(
            bg_span.text().len() > 2,
            "bg span should extend through padding, got: {:?}",
            bg_span.text()
        );
    }

    #[test]
    fn bordered_gap_inserts_empty_bordered_lines_between_children() {
        let mut a = StubComponent {
            lines: vec![Line::new("a")],
        };
        let mut b = StubComponent {
            lines: vec![Line::new("b")],
        };
        let mut container = Container::new(vec![&mut a, &mut b])
            .border_color(Color::Grey)
            .gap(1);
        let context = RenderContext::new((20, 10));
        let lines = container.render(&context);
        // top border + blank + "a" + gap_blank + "b" + bottom border = 6
        assert_eq!(lines.len(), 6);
        // The gap line should be a bordered empty line
        let gap_line = lines[3].plain_text();
        assert!(gap_line.starts_with('│'), "gap: {gap_line}");
        assert!(gap_line.ends_with('│'), "gap: {gap_line}");
    }

    #[test]
    fn top_and_bottom_border_have_equal_visual_width() {
        let mut a = StubComponent {
            lines: vec![Line::new("x")],
        };
        let mut container = Container::new(vec![&mut a])
            .title(" Config ")
            .border_color(Color::Grey);
        let context = RenderContext::new((40, 10));
        let lines = container.render(&context);
        let top = lines.first().unwrap().plain_text();
        let bottom = lines.last().unwrap().plain_text();
        assert_eq!(
            UnicodeWidthStr::width(top.as_str()),
            UnicodeWidthStr::width(bottom.as_str()),
            "top ({top}) and bottom ({bottom}) border should have equal visual width"
        );
    }

    #[test]
    fn no_border_options_renders_like_before() {
        let mut a = StubComponent {
            lines: vec![Line::new("a")],
        };
        let mut b = StubComponent {
            lines: vec![Line::new("b")],
        };
        let mut container = Container::new(vec![&mut a, &mut b]);
        let context = RenderContext::new((80, 24));
        let lines = container.render(&context);
        assert_eq!(lines, vec![Line::new("a"), Line::new("b")]);
    }
}
