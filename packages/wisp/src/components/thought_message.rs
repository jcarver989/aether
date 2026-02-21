use crate::tui::theme::Theme;
use crate::tui::{Component, Line, RenderContext};

pub struct ThoughtMessage<'a> {
    pub text: &'a str,
}

impl ThoughtMessage<'_> {
    fn format_with_theme(text: &str, prefix: bool, theme: &Theme) -> Line {
        let mut line = Line::default();
        if !prefix {
            line.push_styled(text, theme.muted);
            return line;
        }

        line.push_styled("Thought:", theme.info);
        line.push_text(" ");
        line.push_styled(text, theme.muted);
        line
    }

    fn format_lines(text: &str, theme: &Theme) -> Vec<Line> {
        let mut lines = text.lines();
        let Some(first) = lines.next() else {
            return vec![];
        };

        let mut formatted = vec![Self::format_with_theme(first, true, theme)];
        formatted.extend(lines.map(|line| Self::format_with_theme(line, false, theme)));
        formatted
    }
}

impl Component for ThoughtMessage<'_> {
    fn render(&mut self, context: &RenderContext) -> Vec<Line> {
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
    fn renders_prefixed_thought_line() {
        let mut component = ThoughtMessage { text: "check plan" };
        let context = RenderContext::new((80, 24));
        let lines = component.render(&context);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("Thought:"));
        assert!(lines[0].plain_text().contains("check plan"));
    }

    #[test]
    fn prefixes_only_first_line_for_multiline_thought() {
        let mut component = ThoughtMessage {
            text: "line one\nline two",
        };
        let context = RenderContext::new((80, 24));
        let lines = component.render(&context);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("Thought:"));
        assert!(lines[0].plain_text().contains("line one"));
        assert!(!lines[1].plain_text().contains("Thought:"));
        assert!(lines[1].plain_text().contains("line two"));
    }

    #[test]
    fn wrapped_continuation_rows_remain_muted() {
        let mut component = ThoughtMessage {
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
                    .all(|span| span.style().fg == Some(context.theme.muted))
            );
        }
    }
}
