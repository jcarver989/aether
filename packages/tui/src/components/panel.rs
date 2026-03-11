use crate::{Color, Line, Style, ViewContext};
use unicode_width::UnicodeWidthStr;

/// Width consumed by left ("│ ") and right (" │") borders.
pub const BORDER_H_PAD: u16 = 4;

/// A bordered panel for wrapping content blocks with title/footer chrome.
///
/// For borderless stacking with cursor tracking, use [`Layout`](super::layout::Layout).
///
/// # Example
///
/// ```
/// use tui::{Panel, Line, ViewContext};
///
/// let mut panel = Panel::new(tui::Color::Grey)
///     .title(" Settings ")
///     .footer("[Enter] Save [Esc] Cancel")
///     .gap(1);
///
/// panel.push(vec![Line::new("Name: Example")]);
/// panel.push(vec![Line::new("Value: 42")]);
///
/// let ctx = ViewContext::new((40, 20));
/// let lines = panel.render(&ctx);
/// ```
pub struct Panel {
    blocks: Vec<Vec<Line>>,
    title: Option<String>,
    footer: Option<String>,
    border_color: Color,
    fill_height: Option<usize>,
    gap: usize,
}

impl Panel {
    pub fn new(border_color: Color) -> Self {
        Self {
            blocks: Vec::new(),
            title: None,
            footer: None,
            border_color,
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

    pub fn fill_height(mut self, h: usize) -> Self {
        self.fill_height = Some(h);
        self
    }

    pub fn gap(mut self, lines: usize) -> Self {
        self.gap = lines;
        self
    }

    pub fn push(&mut self, block: Vec<Line>) {
        self.blocks.push(block);
    }

    /// Inner content width when borders are active.
    pub fn inner_width(total_width: u16) -> u16 {
        total_width.saturating_sub(BORDER_H_PAD)
    }

    /// Render blocks with borders/chrome.
    pub fn render(&self, context: &ViewContext) -> Vec<Line> {
        let width = context.size.width as usize;
        let inner_width = width.saturating_sub(BORDER_H_PAD as usize);
        let border_style = Style::fg(self.border_color);

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

        // ── Wrap pre-rendered blocks in borders ──
        for (i, block) in self.blocks.iter().enumerate() {
            if i > 0 {
                for _ in 0..self.gap {
                    lines.push(empty_border_line(inner_width));
                }
            }
            for cl in block {
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

    #[test]
    fn title_renders_top_border_with_title_text() {
        let mut container = Panel::new(Color::Grey).title(" Config ");
        container.push(vec![Line::new("x")]);
        let context = ViewContext::new((30, 10));
        let lines = container.render(&context);
        let top = lines[0].plain_text();
        assert!(top.starts_with("┌─ Config "), "top: {top}");
        assert!(top.ends_with('┐'), "top: {top}");
    }

    #[test]
    fn footer_renders_footer_and_bottom_border() {
        let mut container = Panel::new(Color::Grey).footer("[Esc] Close");
        container.push(vec![Line::new("x")]);
        let context = ViewContext::new((30, 10));
        let lines = container.render(&context);
        let last = lines.last().unwrap().plain_text();
        assert!(last.starts_with('└'), "last: {last}");
        assert!(last.ends_with('┘'), "last: {last}");
        let footer = lines[lines.len() - 2].plain_text();
        assert!(footer.contains("[Esc] Close"), "footer: {footer}");
    }

    #[test]
    fn fill_height_pads_with_empty_bordered_rows() {
        let mut container = Panel::new(Color::Grey)
            .title(" T ")
            .footer("F")
            .fill_height(10);
        container.push(vec![Line::new("x")]);
        let context = ViewContext::new((30, 10));
        let lines = container.render(&context);
        assert_eq!(lines.len(), 10, "should fill to exactly 10 lines");
    }

    #[test]
    fn border_color_styles_border_lines() {
        let mut container = Panel::new(Color::Cyan).title(" T ");
        container.push(vec![Line::new("x")]);
        let context = ViewContext::new((30, 10));
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
        let mut container = Panel::new(Color::Grey);
        container.push(vec![Line::with_style("hi", Style::default().bg_color(bg))]);
        let context = ViewContext::new((20, 10));
        let lines = container.render(&context);
        // Content row (top border + blank + first content = index 2)
        let content_row = &lines[2];
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
        let mut container = Panel::new(Color::Grey).gap(1);
        container.push(vec![Line::new("a")]);
        container.push(vec![Line::new("b")]);
        let context = ViewContext::new((20, 10));
        let lines = container.render(&context);
        // top border + blank + "a" + gap_blank + "b" + bottom border = 6
        assert_eq!(lines.len(), 6);
        let gap_line = lines[3].plain_text();
        assert!(gap_line.starts_with('│'), "gap: {gap_line}");
        assert!(gap_line.ends_with('│'), "gap: {gap_line}");
    }

    #[test]
    fn top_and_bottom_border_have_equal_visual_width() {
        let mut container = Panel::new(Color::Grey).title(" Config ");
        container.push(vec![Line::new("x")]);
        let context = ViewContext::new((40, 10));
        let lines = container.render(&context);
        let top = lines.first().unwrap().plain_text();
        let bottom = lines.last().unwrap().plain_text();
        assert_eq!(
            UnicodeWidthStr::width(top.as_str()),
            UnicodeWidthStr::width(bottom.as_str()),
            "top ({top}) and bottom ({bottom}) border should have equal visual width"
        );
    }
}
