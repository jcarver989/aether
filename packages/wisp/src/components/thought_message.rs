use crate::tui::screen::Style;
use crate::tui::theme::Theme;
use crate::tui::{Component, Line, RenderContext};

pub struct ThoughtMessage<'a> {
    pub text: &'a str,
}

impl ThoughtMessage<'_> {
    fn format_line(text: &str, theme: &Theme) -> Line {
        let mut line = Line::default();
        line.push_styled("│ ", theme.muted());
        line.push_with_style(text, Style::fg(theme.muted()));
        line
    }

    fn format_lines(text: &str, theme: &Theme) -> Vec<Line> {
        text.lines()
            .map(|line| Self::format_line(line, theme))
            .collect()
    }
}

impl Component for ThoughtMessage<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        if self.text.is_empty() {
            return vec![];
        }

        Self::format_lines(self.text, &context.theme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::soft_wrap::soft_wrap_line;

    #[test]
    fn renders_border_prefixed_thought_line() {
        let component = ThoughtMessage { text: "check plan" };
        let context = RenderContext::new((80, 24));
        let lines = component.render(&context);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().starts_with("│ "));
        assert!(lines[0].plain_text().contains("check plan"));
    }

    #[test]
    fn prefixes_all_lines_with_border() {
        let component = ThoughtMessage {
            text: "line one\nline two",
        };
        let context = RenderContext::new((80, 24));
        let lines = component.render(&context);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().starts_with("│ "));
        assert!(lines[0].plain_text().contains("line one"));
        assert!(lines[1].plain_text().starts_with("│ "));
        assert!(lines[1].plain_text().contains("line two"));
    }

    #[test]
    fn wrapped_continuation_rows_remain_muted() {
        let component = ThoughtMessage {
            text: "abcdefghijklmnopqrstuvwxyz",
        };
        let context = RenderContext::new((80, 24));
        let lines = component.render(&context);
        let wrapped = soft_wrap_line(&lines[0], 12);
        assert!(wrapped.len() > 1);

        for row in wrapped.iter().skip(1) {
            assert!(!row.spans().is_empty());
            assert!(
                row.spans()
                    .iter()
                    .all(|span| span.style().fg == Some(context.theme.muted()))
            );
        }
    }
}
