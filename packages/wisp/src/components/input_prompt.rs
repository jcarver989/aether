use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;

pub struct InputPrompt<'a> {
    pub input: &'a str,
}

impl Component for InputPrompt<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        let width = context.size.0 as usize;
        if width < 4 {
            // Too narrow for borders, fall back to minimal
            return vec![Line::new(format!("> {}", self.input))];
        }

        let inner_width = width - 2; // space between │ and │

        // Top border: ╭──...──╮
        let top = format!("╭{}╮", "─".repeat(inner_width));
        let top_styled = top.with(context.theme.muted);

        // Middle: │ > input_text ... │
        let border_left = "│".with(context.theme.muted);
        let border_right = "│".with(context.theme.muted);
        let prompt_marker = "> ".with(context.theme.primary);
        let input_text = self.input.with(context.theme.text_primary);
        // Build middle line: colored borders + colored prompt + colored input + padding
        let prefix_len = 1 + 2 + self.input.len(); // space + "> " + input
        let pad_len = inner_width.saturating_sub(prefix_len);
        let middle = format!(
            "{border_left} {prompt_marker}{input_text}{:pad_len$}{border_right}",
            "",
        );

        // Bottom border: ╰──...──╯
        let bottom = format!("╰{}╯", "─".repeat(inner_width));
        let bottom_styled = bottom.with(context.theme.muted);

        vec![
            Line::new(format!("{top_styled}")),
            Line::new(middle),
            Line::new(format!("{bottom_styled}")),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_three_lines() {
        let prompt = InputPrompt { input: "" };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn top_border_contains_box_chars() {
        let prompt = InputPrompt { input: "" };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[0].as_str().contains("╭"));
        assert!(lines[0].as_str().contains("╮"));
    }

    #[test]
    fn bottom_border_contains_box_chars() {
        let prompt = InputPrompt { input: "" };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[2].as_str().contains("╰"));
        assert!(lines[2].as_str().contains("╯"));
    }

    #[test]
    fn middle_line_contains_prompt() {
        let prompt = InputPrompt { input: "" };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[1].as_str().contains("> "));
        assert!(lines[1].as_str().contains("│"));
    }

    #[test]
    fn renders_input_text() {
        let prompt = InputPrompt { input: "hello" };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[1].as_str().contains("hello"));
    }

    #[test]
    fn renders_consistently() {
        let prompt = InputPrompt { input: "test" };
        let ctx = RenderContext::new((80, 24));
        let a = prompt.render(&ctx);
        let b = prompt.render(&ctx);
        assert_eq!(a, b);
    }

    #[test]
    fn adapts_to_terminal_width() {
        let prompt = InputPrompt { input: "" };
        let narrow = RenderContext::new((40, 24));
        let wide = RenderContext::new((120, 24));
        let narrow_lines = prompt.render(&narrow);
        let wide_lines = prompt.render(&wide);
        // Both should produce 3 lines but different widths
        assert_eq!(narrow_lines.len(), 3);
        assert_eq!(wide_lines.len(), 3);
        // Wide border should be longer than narrow
        assert!(wide_lines[0].as_str().len() > narrow_lines[0].as_str().len());
    }
}
