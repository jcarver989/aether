use super::screen::Line;
use unicode_width::UnicodeWidthChar;

pub fn display_width_text(s: &str) -> usize {
    s.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

pub fn display_width_line(line: &Line) -> usize {
    line.spans()
        .iter()
        .map(|span| display_width_text(span.text()))
        .sum()
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

    for span in line.spans() {
        let text = span.text();
        let style = span.style();
        let mut start = 0;

        for (i, ch) in text.char_indices() {
            if ch == '\n' {
                if start < i {
                    current.push_with_style(&text[start..i], style);
                }
                rows.push(current);
                current = Line::default();
                current_width = 0;
                start = i + ch.len_utf8();
                continue;
            }

            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if ch_width > 0 && current_width + ch_width > max_width && current_width > 0 {
                if start < i {
                    current.push_with_style(&text[start..i], style);
                }
                rows.push(current);
                current = Line::default();
                current_width = 0;
                start = i;
            }
            current_width += ch_width;
        }

        if start < text.len() {
            current.push_with_style(&text[start..], style);
        }
    }

    rows.push(current);
    if rows.is_empty() {
        rows.push(Line::new(""));
    }
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
}
