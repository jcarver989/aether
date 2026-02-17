use crate::tui::soft_wrap::{display_width_ansi, soft_wrap_str};
use crate::tui::{Component, Line, RenderContext};
use crossterm::style::Stylize;
use unicode_width::UnicodeWidthChar;

pub struct InputPrompt<'a> {
    pub input: &'a str,
    pub cursor_index: usize,
}

pub struct InputPromptLayout {
    pub lines: Vec<Line>,
    /// Cursor row within `lines` (0-based).
    pub cursor_row: usize,
    /// Cursor column on that row (0-based).
    pub cursor_col: u16,
}

impl InputPrompt<'_> {
    pub fn layout(&self, context: &RenderContext) -> InputPromptLayout {
        let width = context.size.0 as usize;
        let cursor_index = clamp_to_char_boundary(self.input, self.cursor_index);
        let cursor_display_width = plain_display_width(&self.input[..cursor_index]);
        let styled_input = style_input(self.input, context);

        if width < 4 {
            let line = format!("> {styled_input}");
            let total_col = 2 + cursor_display_width;
            let (cursor_row, cursor_col) = if width == 0 {
                (0, 0)
            } else {
                (total_col / width, (total_col % width) as u16)
            };
            return InputPromptLayout {
                lines: vec![Line::new(line)],
                cursor_row,
                cursor_col,
            };
        }

        let inner_width = width - 2; // space between │ and │
        // " " + marker area ("> " or "  ")
        let content_width = inner_width.saturating_sub(3).max(1);
        let wrapped_chunks = soft_wrap_str(&styled_input, content_width as u16);

        let cursor_content_row = cursor_display_width / content_width;
        let cursor_content_col = cursor_display_width % content_width;

        // Ensure we always render enough rows to place the cursor safely.
        let content_rows = wrapped_chunks.len().max(cursor_content_row + 1);

        let top = format!("╭{}╮", "─".repeat(inner_width)).with(context.theme.muted);
        let bottom = format!("╰{}╯", "─".repeat(inner_width)).with(context.theme.muted);
        let border_left = "│".with(context.theme.muted);
        let border_right = "│".with(context.theme.muted);
        let first_prompt = format!("{}", "> ".with(context.theme.primary));
        let continuation_prompt = format!("{}", "  ".with(context.theme.muted));

        let mut lines = Vec::with_capacity(content_rows + 2);
        lines.push(Line::new(format!("{top}")));

        for row in 0..content_rows {
            let chunk = wrapped_chunks.get(row).map_or("", String::as_str);
            let prompt = if row == 0 {
                &first_prompt
            } else {
                &continuation_prompt
            };
            let pad_len = content_width.saturating_sub(display_width_ansi(chunk));
            let middle = format!("{border_left} {prompt}{chunk}{:pad_len$}{border_right}", "");
            lines.push(Line::new(middle));
        }

        lines.push(Line::new(format!("{bottom}")));

        InputPromptLayout {
            lines,
            cursor_row: 1 + cursor_content_row,
            // "│ > " (or "│   ") takes 4 visual columns.
            cursor_col: (4 + cursor_content_col) as u16,
        }
    }
}

impl Component for InputPrompt<'_> {
    fn render(&self, context: &RenderContext) -> Vec<Line> {
        self.layout(context).lines
    }
}

fn style_input(input: &str, context: &RenderContext) -> String {
    if !input.contains('@') {
        return input.with(context.theme.text_primary).to_string();
    }
    style_mentions(input, context)
}

fn style_mentions(input: &str, context: &RenderContext) -> String {
    let mut styled = String::new();
    let mut last_pos = 0;

    for (at_pos, _) in input.match_indices('@') {
        if at_pos < last_pos {
            continue;
        }

        styled.push_str(
            &input[last_pos..at_pos]
                .with(context.theme.text_primary)
                .to_string(),
        );

        let mention_end = input[at_pos..]
            .find(' ')
            .map(|i| at_pos + i)
            .unwrap_or(input.len());
        styled.push_str(
            &input[at_pos..mention_end]
                .with(context.theme.info)
                .to_string(),
        );
        last_pos = mention_end;
    }

    if last_pos < input.len() {
        styled.push_str(
            &input[last_pos..]
                .with(context.theme.text_primary)
                .to_string(),
        );
    }

    styled
}

fn plain_display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

fn clamp_to_char_boundary(text: &str, mut idx: usize) -> usize {
    idx = idx.min(text.len());
    while !text.is_char_boundary(idx) {
        idx = idx.saturating_sub(1);
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_three_lines() {
        let prompt = InputPrompt {
            input: "",
            cursor_index: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn top_border_contains_box_chars() {
        let prompt = InputPrompt {
            input: "",
            cursor_index: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[0].as_str().contains("╭"));
        assert!(lines[0].as_str().contains("╮"));
    }

    #[test]
    fn bottom_border_contains_box_chars() {
        let prompt = InputPrompt {
            input: "",
            cursor_index: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[2].as_str().contains("╰"));
        assert!(lines[2].as_str().contains("╯"));
    }

    #[test]
    fn middle_line_contains_prompt() {
        let prompt = InputPrompt {
            input: "",
            cursor_index: 0,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[1].as_str().contains("> "));
        assert!(lines[1].as_str().contains("│"));
    }

    #[test]
    fn renders_input_text() {
        let prompt = InputPrompt {
            input: "hello",
            cursor_index: 5,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[1].as_str().contains("hello"));
    }

    #[test]
    fn renders_consistently() {
        let prompt = InputPrompt {
            input: "test",
            cursor_index: 4,
        };
        let ctx = RenderContext::new((80, 24));
        let a = prompt.render(&ctx);
        let b = prompt.render(&ctx);
        assert_eq!(a, b);
    }

    #[test]
    fn adapts_to_terminal_width() {
        let prompt = InputPrompt {
            input: "",
            cursor_index: 0,
        };
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

    #[test]
    fn wraps_long_input_inside_box() {
        let prompt = InputPrompt {
            input: "this is a very long input that should wrap",
            cursor_index: 41,
        };
        let ctx = RenderContext::new((20, 24));
        let lines = prompt.render(&ctx);
        assert!(lines.len() > 3);
        assert!(lines.iter().all(|line| line.as_str().contains("│")
            || line.as_str().contains("╭")
            || line.as_str().contains("╰")));
    }

    #[test]
    fn mention_and_plain_text_both_render() {
        let prompt = InputPrompt {
            input: "@main.rs explain this",
            cursor_index: 20,
        };
        let ctx = RenderContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[1].as_str().contains("@main.rs"));
        assert!(lines[1].as_str().contains("explain this"));
    }
}
