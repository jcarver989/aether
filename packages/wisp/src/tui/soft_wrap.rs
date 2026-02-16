use super::screen::Line;
use unicode_width::UnicodeWidthChar;

pub fn display_width_ansi(s: &str) -> usize {
    let mut width = 0;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
            continue;
        }
        width += UnicodeWidthChar::width(ch).unwrap_or(0);
    }

    width
}

pub fn soft_wrap_line(line: &Line, width: u16) -> Vec<Line> {
    soft_wrap_str(line.as_str(), width)
        .into_iter()
        .map(Line::new)
        .collect()
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

pub fn soft_wrap_str(s: &str, width: u16) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }

    let max_width = width as usize;
    if max_width == 0 {
        return vec![s.to_string()];
    }

    let mut rows = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            current.push(ch);
            current.push(chars.next().expect("peeked '[' must exist"));
            for c in chars.by_ref() {
                current.push(c);
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
            continue;
        }

        if ch == '\n' {
            rows.push(current);
            current = String::new();
            current_width = 0;
            continue;
        }

        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if ch_width > 0 && current_width + ch_width > max_width && !current.is_empty() {
            rows.push(current);
            current = String::new();
            current_width = 0;
        }

        current.push(ch);
        current_width += ch_width;
    }

    rows.push(current);
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_ascii_to_width() {
        let rows = soft_wrap_str("abcdef", 3);
        assert_eq!(rows, vec!["abc", "def"]);
    }

    #[test]
    fn ansi_is_zero_width_for_measurement() {
        let red = "\x1b[31mhello\x1b[39m";
        assert_eq!(display_width_ansi(red), 5);
    }

    #[test]
    fn wraps_with_ansi_sequences() {
        let s = "\x1b[31mabcdef\x1b[39m";
        let rows = soft_wrap_str(s, 3);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].contains("abc"));
        assert!(rows[1].contains("def"));
    }

    #[test]
    fn counts_wide_unicode() {
        assert_eq!(display_width_ansi("中a"), 3);
        let rows = soft_wrap_str("中ab", 3);
        assert_eq!(rows, vec!["中a", "b"]);
    }
}
