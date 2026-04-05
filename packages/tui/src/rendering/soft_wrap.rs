use std::borrow::Cow;

use super::line::Line;
use unicode_width::UnicodeWidthChar;

/// Truncates text to fit within `max_width` display columns, appending "..." if truncated.
/// Returns the original string borrowed when no truncation is needed.
pub fn truncate_text(text: &str, max_width: usize) -> Cow<'_, str> {
    const ELLIPSIS: &str = "...";
    const ELLIPSIS_WIDTH: usize = 3;

    if max_width == 0 {
        return Cow::Borrowed("");
    }

    let use_ellipsis = max_width >= ELLIPSIS_WIDTH;
    let budget = if use_ellipsis { max_width - ELLIPSIS_WIDTH } else { max_width };

    let mut width = 0;
    let mut fit_end = 0; // byte offset after last char fitting within budget

    for (i, ch) in text.char_indices() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + cw > max_width {
            return if use_ellipsis {
                Cow::Owned(format!("{}{ELLIPSIS}", &text[..fit_end]))
            } else {
                Cow::Owned(text[..fit_end].to_owned())
            };
        }
        width += cw;
        if width <= budget {
            fit_end = i + ch.len_utf8();
        }
    }

    Cow::Borrowed(text)
}

/// Pads `text` with trailing spaces to reach `target_width` display columns.
/// Returns the original text unchanged if it already meets or exceeds the target.
pub fn pad_text_to_width(text: &str, target_width: usize) -> Cow<'_, str> {
    let current = display_width_text(text);
    if current >= target_width {
        Cow::Borrowed(text)
    } else {
        let padding = target_width - current;
        Cow::Owned(format!("{text}{}", " ".repeat(padding)))
    }
}

pub fn display_width_text(s: &str) -> usize {
    s.chars().map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0)).sum()
}

pub fn display_width_line(line: &Line) -> usize {
    line.spans().iter().map(|span| display_width_text(span.text())).sum()
}

/// Truncates a styled line to fit within `max_width` display columns.
///
/// Walks spans tracking cumulative display width, slicing at the character
/// boundary where the budget is exhausted. No ellipsis is appended — callers
/// can pad with [`Line::extend_bg_to_width`] if needed.
pub fn truncate_line(line: &Line, max_width: usize) -> Line {
    if max_width == 0 {
        return Line::default();
    }

    let mut result = Line::default();
    let mut remaining = max_width;

    for span in line.spans() {
        if remaining == 0 {
            break;
        }

        let text = span.text();
        let style = span.style();
        let mut byte_end = 0;
        let mut col = 0;

        for (i, ch) in text.char_indices() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if col + cw > remaining {
                break;
            }
            col += cw;
            byte_end = i + ch.len_utf8();
        }

        if byte_end > 0 {
            result.push_with_style(&text[..byte_end], style);
        }
        remaining -= col;
    }

    result
}

pub fn soft_wrap_line(line: &Line, width: u16) -> Vec<Line> {
    if line.is_empty() {
        return vec![Line::new("")];
    }

    let max_width = width as usize;
    if max_width == 0 {
        return vec![line.clone()];
    }

    let mut rows = Vec::new();
    let mut current = Line::default();
    let mut current_width = 0usize;
    let mut last_ws: Option<(usize, usize, usize)>; // (byte offset, byte offset after ws, width after ws)

    for span in line.spans() {
        let text = span.text();
        let style = span.style();
        let mut start = 0;
        last_ws = None;

        for (i, ch) in text.char_indices() {
            if ch == '\n' {
                if start < i {
                    current.push_with_style(&text[start..i], style);
                }
                rows.push(current);
                current = Line::default();
                current_width = 0;
                last_ws = None;
                start = i + ch.len_utf8();
                continue;
            }

            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if ch_width > 0 && current_width + ch_width > max_width && current_width > 0 {
                let (break_at, skip_to, new_width) = if let Some((ws_pos, ws_end, width_after_ws)) = last_ws.take() {
                    (ws_pos, ws_end, current_width - width_after_ws)
                } else {
                    (i, i, 0)
                };

                if start < break_at {
                    current.push_with_style(&text[start..break_at], style);
                }
                rows.push(current);
                current = Line::default();
                current_width = new_width;
                if skip_to < i {
                    current.push_with_style(&text[skip_to..i], style);
                }
                start = i;
            }
            current_width += ch_width;
            if ch.is_whitespace() {
                last_ws = Some((i, i + ch.len_utf8(), current_width));
            }
        }

        if start < text.len() {
            current.push_with_style(&text[start..], style);
        }
    }

    rows.push(current);
    rows
}

pub fn soft_wrap_lines_with_map(lines: &[Line], width: u16) -> (Vec<Line>, Vec<usize>) {
    let mut out = Vec::new();
    let mut starts = Vec::with_capacity(lines.len());

    for line in lines {
        starts.push(out.len());
        out.extend(soft_wrap_line(line, width));
    }

    (out, starts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::style::Color;

    #[test]
    fn wraps_ascii_to_width() {
        let rows = soft_wrap_line(&Line::new("abcdef"), 3);
        assert_eq!(rows, vec![Line::new("abc"), Line::new("def")]);
    }

    #[test]
    fn display_width_ignores_style() {
        let mut line = Line::default();
        line.push_styled("he", Color::Red);
        line.push_text("llo");
        assert_eq!(display_width_line(&line), 5);
    }

    #[test]
    fn wraps_preserving_style_spans() {
        let line = Line::styled("abcdef", Color::Red);
        let rows = soft_wrap_line(&line, 3);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].plain_text(), "abc");
        assert_eq!(rows[1].plain_text(), "def");
        assert_eq!(rows[0].spans().len(), 1);
        assert_eq!(rows[1].spans().len(), 1);
        assert_eq!(rows[0].spans()[0].style().fg, Some(Color::Red));
        assert_eq!(rows[1].spans()[0].style().fg, Some(Color::Red));
    }

    #[test]
    fn counts_wide_unicode() {
        assert_eq!(display_width_text("中a"), 3);
        let rows = soft_wrap_line(&Line::new("中ab"), 3);
        assert_eq!(rows, vec![Line::new("中a"), Line::new("b")]);
    }

    #[test]
    fn wraps_multi_span_line_mid_span() {
        let mut line = Line::default();
        line.push_styled("ab", Color::Red);
        line.push_styled("cd", Color::Blue);
        line.push_styled("ef", Color::Green);
        let rows = soft_wrap_line(&line, 3);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].plain_text(), "abc");
        assert_eq!(rows[1].plain_text(), "def");
        // First row: "ab" (Red) + "c" (Blue)
        assert_eq!(rows[0].spans().len(), 2);
        assert_eq!(rows[0].spans()[0].style().fg, Some(Color::Red));
        assert_eq!(rows[0].spans()[1].style().fg, Some(Color::Blue));
        // Second row: "d" (Blue) + "ef" (Green)
        assert_eq!(rows[1].spans().len(), 2);
        assert_eq!(rows[1].spans()[0].style().fg, Some(Color::Blue));
        assert_eq!(rows[1].spans()[1].style().fg, Some(Color::Green));
    }

    #[test]
    fn wraps_line_with_embedded_newlines() {
        let line = Line::new("abc\ndef");
        let rows = soft_wrap_line(&line, 80);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].plain_text(), "abc");
        assert_eq!(rows[1].plain_text(), "def");
    }

    #[test]
    fn pad_text_pads_ascii_to_target_width() {
        let result = pad_text_to_width("hello", 10);
        assert_eq!(result, "hello     ");
        assert_eq!(display_width_text(&result), 10);
    }

    #[test]
    fn pad_text_returns_borrowed_when_already_wide_enough() {
        let result = pad_text_to_width("hello", 5);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");

        let result = pad_text_to_width("hello", 3);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn pad_text_handles_wide_unicode() {
        // "中" is 2 display columns wide
        let result = pad_text_to_width("中a", 6);
        assert_eq!(display_width_text(&result), 6);
        assert_eq!(result, "中a   "); // 2+1 = 3 cols, need 3 spaces
    }

    #[test]
    fn truncate_text_fits_within_width() {
        assert_eq!(truncate_text("hello", 10), "hello");
        assert_eq!(truncate_text("hello world", 8), "hello...");
        assert_eq!(truncate_text("hello", 5), "hello");
        assert_eq!(truncate_text("hello", 4), "h...");
    }

    #[test]
    fn truncate_text_handles_wide_unicode() {
        // Chinese characters are 2 columns wide
        assert_eq!(truncate_text("中文字", 5), "中..."); // 6 cols -> truncate to 2+3=5
        assert_eq!(truncate_text("中ab", 4), "中ab"); // 2+1+1=4, fits exactly
        assert_eq!(truncate_text("中abc", 4), "..."); // 5 cols, only ellipsis fits in 4
        assert_eq!(truncate_text("中abcde", 6), "中a..."); // 7 cols -> truncate to 2+1+3=6
    }

    #[test]
    fn truncate_text_handles_zero_width() {
        assert_eq!(truncate_text("hello", 0), "");
    }

    #[test]
    fn truncate_text_max_width_1() {
        let result = truncate_text("hello", 1);
        assert!(
            display_width_text(&result) <= 1,
            "Expected width <= 1, got '{}' (width {})",
            result,
            display_width_text(&result),
        );
        assert_eq!(result, "h");
    }

    #[test]
    fn truncate_text_max_width_2() {
        let result = truncate_text("hello", 2);
        assert!(
            display_width_text(&result) <= 2,
            "Expected width <= 2, got '{}' (width {})",
            result,
            display_width_text(&result),
        );
        assert_eq!(result, "he");
    }

    #[test]
    fn truncate_line_returns_short_lines_unchanged() {
        let line = Line::new("short");
        let result = truncate_line(&line, 20);
        assert_eq!(result.plain_text(), "short");
    }

    #[test]
    fn truncate_line_trims_long_styled_lines() {
        let mut line = Line::default();
        line.push_styled("hello", Color::Red);
        line.push_styled(" world", Color::Blue);
        let result = truncate_line(&line, 7);
        assert_eq!(result.plain_text(), "hello w");
        assert_eq!(result.spans().len(), 2);
        assert_eq!(result.spans()[0].style().fg, Some(Color::Red));
        assert_eq!(result.spans()[1].style().fg, Some(Color::Blue));
    }

    #[test]
    fn truncate_line_handles_mid_span_cut() {
        let line = Line::styled("abcdefgh", Color::Green);
        let result = truncate_line(&line, 4);
        assert_eq!(result.plain_text(), "abcd");
        assert_eq!(result.spans()[0].style().fg, Some(Color::Green));
    }

    #[test]
    fn truncate_line_handles_wide_unicode_at_boundary() {
        // "中" is 2 display columns, "文" is 2.
        // Budget of 3: "中"(2) fits, "文"(2) would exceed (2+2=4>3), so stop.
        let line = Line::new("中文x");
        let result = truncate_line(&line, 3);
        assert_eq!(result.plain_text(), "中");

        // Budget of 4: "中"(2) + "文"(2) = 4, fits exactly.
        let result = truncate_line(&line, 4);
        assert_eq!(result.plain_text(), "中文");

        // Budget of 5: all fit: 2+2+1=5.
        let result = truncate_line(&line, 5);
        assert_eq!(result.plain_text(), "中文x");
    }

    #[test]
    fn truncate_line_zero_width_returns_empty() {
        let line = Line::new("hello");
        let result = truncate_line(&line, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn wraps_at_word_boundary() {
        let rows = soft_wrap_line(&Line::new("hello world"), 7);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].plain_text(), "hello");
        assert_eq!(rows[1].plain_text(), "world");
    }

    #[test]
    fn wraps_multiple_words() {
        let rows = soft_wrap_line(&Line::new("hello world foo"), 12);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].plain_text(), "hello world");
        assert_eq!(rows[1].plain_text(), "foo");
    }

    #[test]
    fn falls_back_to_char_break_without_whitespace() {
        let rows = soft_wrap_line(&Line::new("superlongword next"), 5);
        assert_eq!(rows[0].plain_text(), "super");
        assert_eq!(rows[1].plain_text(), "longw");
        assert_eq!(rows[2].plain_text(), "ord");
        assert_eq!(rows[3].plain_text(), "next");
    }

    #[test]
    fn wraps_at_word_boundary_with_styled_spans() {
        let line = Line::styled("hello world", Color::Red);
        let rows = soft_wrap_line(&line, 7);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].plain_text(), "hello");
        assert_eq!(rows[1].plain_text(), "world");
        assert_eq!(rows[0].spans()[0].style().fg, Some(Color::Red));
        assert_eq!(rows[1].spans()[0].style().fg, Some(Color::Red));
    }

    #[test]
    fn wraps_across_spans_without_panic() {
        let mut line = Line::default();
        line.push_styled("hello ", Color::Red);
        line.push_styled("world this is long", Color::Blue);
        let rows = soft_wrap_line(&line, 10);
        assert_eq!(rows[0].plain_text(), "hello worl");
        assert_eq!(rows[1].plain_text(), "d this is");
        assert_eq!(rows[2].plain_text(), "long");
    }
}
