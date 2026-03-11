use crate::tui::{Line, ViewContext};
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
    pub fn layout(&self, context: &ViewContext) -> InputPromptLayout {
        let width = usize::from(context.size.width);
        let cursor_index = clamp_to_char_boundary(self.input, self.cursor_index);
        let cursor_display_width = plain_display_width(&self.input[..cursor_index]);
        let styled_input = style_input(self.input, context);

        if width < 4 {
            let mut line = Line::new("> ");
            line.append_line(&styled_input);
            let total_col = 2 + cursor_display_width;
            let (cursor_row, cursor_col) = if width == 0 {
                (0, 0)
            } else {
                (
                    total_col / width,
                    u16::try_from(total_col % width).unwrap_or(u16::MAX),
                )
            };
            return InputPromptLayout {
                lines: vec![line],
                cursor_row,
                cursor_col,
            };
        }

        let inner_width = width - 2; // space between │ and │
        // " " + marker area ("> " or "  ")
        let content_width = inner_width.saturating_sub(3).max(1);
        let wrapped_chunks =
            styled_input.soft_wrap(u16::try_from(content_width).unwrap_or(u16::MAX));

        let cursor_content_row = cursor_display_width / content_width;
        let cursor_content_col = cursor_display_width % content_width;

        // Ensure we always render enough rows to place the cursor safely.
        let content_rows = wrapped_chunks.len().max(cursor_content_row + 1);

        let mut lines = Vec::with_capacity(content_rows + 2);
        lines.push(Line::styled(
            format!("╭{}╮", "─".repeat(inner_width)),
            context.theme.muted(),
        ));

        for row in 0..content_rows {
            let chunk = wrapped_chunks.get(row).cloned().unwrap_or_default();
            let pad_len = content_width.saturating_sub(chunk.display_width());
            let mut middle = Line::default();
            middle.push_styled("│", context.theme.muted());
            middle.push_text(" ");
            if row == 0 {
                middle.push_styled("> ", context.theme.primary());
            } else {
                middle.push_styled("  ", context.theme.muted());
            }
            middle.append_line(&chunk);
            middle.push_text(" ".repeat(pad_len));
            middle.push_styled("│", context.theme.muted());
            lines.push(middle);
        }

        lines.push(Line::styled(
            format!("╰{}╯", "─".repeat(inner_width)),
            context.theme.muted(),
        ));

        InputPromptLayout {
            lines,
            cursor_row: 1 + cursor_content_row,
            // "│ > " (or "│   ") takes 4 visual columns.
            cursor_col: u16::try_from(4 + cursor_content_col).unwrap_or(u16::MAX),
        }
    }
}

impl InputPrompt<'_> {
    pub fn render(&self, context: &ViewContext) -> Vec<Line> {
        self.layout(context).lines
    }
}

fn style_input(input: &str, context: &ViewContext) -> Line {
    if !input.contains('@') {
        return Line::styled(input, context.theme.text_primary());
    }
    style_mentions(input, context)
}

fn style_mentions(input: &str, context: &ViewContext) -> Line {
    let mut styled = Line::default();
    let mut last_pos = 0;

    for (at_pos, _) in input.match_indices('@') {
        if at_pos < last_pos {
            continue;
        }

        styled.push_styled(&input[last_pos..at_pos], context.theme.text_primary());

        let mention_end = input[at_pos..]
            .find(' ')
            .map_or(input.len(), |i| at_pos + i);
        styled.push_styled(&input[at_pos..mention_end], context.theme.info());
        last_pos = mention_end;
    }

    if last_pos < input.len() {
        styled.push_styled(&input[last_pos..], context.theme.text_primary());
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
        let ctx = ViewContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn top_border_contains_box_chars() {
        let prompt = InputPrompt {
            input: "",
            cursor_index: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[0].plain_text().contains("╭"));
        assert!(lines[0].plain_text().contains("╮"));
    }

    #[test]
    fn bottom_border_contains_box_chars() {
        let prompt = InputPrompt {
            input: "",
            cursor_index: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[2].plain_text().contains("╰"));
        assert!(lines[2].plain_text().contains("╯"));
    }

    #[test]
    fn middle_line_contains_prompt() {
        let prompt = InputPrompt {
            input: "",
            cursor_index: 0,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[1].plain_text().contains("> "));
        assert!(lines[1].plain_text().contains("│"));
    }

    #[test]
    fn renders_input_text() {
        let prompt = InputPrompt {
            input: "hello",
            cursor_index: 5,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[1].plain_text().contains("hello"));
    }

    #[test]
    fn renders_consistently() {
        let prompt = InputPrompt {
            input: "test",
            cursor_index: 4,
        };
        let ctx = ViewContext::new((80, 24));
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
        let narrow = ViewContext::new((40, 24));
        let wide = ViewContext::new((120, 24));
        let narrow_lines = prompt.render(&narrow);
        let wide_lines = prompt.render(&wide);
        // Both should produce 3 lines but different widths
        assert_eq!(narrow_lines.len(), 3);
        assert_eq!(wide_lines.len(), 3);
        // Wide border should be longer than narrow
        assert!(wide_lines[0].plain_text().len() > narrow_lines[0].plain_text().len());
    }

    #[test]
    fn wraps_long_input_inside_box() {
        let prompt = InputPrompt {
            input: "this is a very long input that should wrap",
            cursor_index: 41,
        };
        let ctx = ViewContext::new((20, 24));
        let lines = prompt.render(&ctx);
        assert!(lines.len() > 3);
        assert!(lines.iter().all(|line| line.plain_text().contains("│")
            || line.plain_text().contains("╭")
            || line.plain_text().contains("╰")));
    }

    #[test]
    fn mention_and_plain_text_both_render() {
        let prompt = InputPrompt {
            input: "@main.rs explain this",
            cursor_index: 20,
        };
        let ctx = ViewContext::new((80, 24));
        let lines = prompt.render(&ctx);
        assert!(lines[1].plain_text().contains("@main.rs"));
        assert!(lines[1].plain_text().contains("explain this"));
    }
}
