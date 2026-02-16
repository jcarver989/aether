use crate::tui::theme::Theme;
use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;

pub struct ThoughtMessage<'a> {
    pub text: &'a str,
}

impl ThoughtMessage<'_> {
    fn format_with_theme(text: &str, prefix: bool, theme: &Theme) -> String {
        let body = text.with(theme.muted);
        if !prefix {
            return format!("{body}");
        }

        let prefix = "Thought:".with(theme.info);
        format!("{prefix} {body}")
    }

    fn format_lines(text: &str, theme: &Theme) -> Vec<String> {
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
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        if self.text.is_empty() {
            return vec![];
        }

        Self::format_lines(self.text, &context.theme)
            .into_iter()
            .map(Line::new)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_prefixed_thought_line() {
        let component = ThoughtMessage { text: "check plan" };
        let context = RenderContext::new((80, 24));
        let lines = component.render(&context);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().contains("Thought:"));
        assert!(lines[0].as_str().contains("check plan"));
    }

    #[test]
    fn prefixes_only_first_line_for_multiline_thought() {
        let component = ThoughtMessage {
            text: "line one\nline two",
        };
        let context = RenderContext::new((80, 24));
        let lines = component.render(&context);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].as_str().contains("Thought:"));
        assert!(lines[0].as_str().contains("line one"));
        assert!(!lines[1].as_str().contains("Thought:"));
        assert!(lines[1].as_str().contains("line two"));
    }
}
