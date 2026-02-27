use crossterm::style::Color;
use syntect::easy::HighlightLines;
use syntect::parsing::SyntaxReference;

use acp_utils::notifications::DiffPreview;

use super::screen::{Line, Span, Style};
use super::syntax::{SYNTECT, find_syntax_for_hint, syntect_to_wisp_style};
use super::theme::Theme;

const MAX_DIFF_LINES: usize = 20;

struct DiffStyle<'a> {
    prefix: &'a str,
    fg: Color,
    bg: Color,
}

/// Render a diff preview with syntax-highlighted removed/added lines.
///
/// Removed lines are shown with a `- ` prefix and red-tinted background.
/// Added lines are shown with a `+ ` prefix and green-tinted background.
pub fn highlight_diff(preview: &DiffPreview, theme: &Theme) -> Vec<Line> {
    let syntax = find_syntax_for_hint(&preview.lang_hint);

    let total = preview.removed.len() + preview.added.len();
    let truncated = total > MAX_DIFF_LINES;
    let budget = if truncated { MAX_DIFF_LINES } else { total };

    // Split budget: show as many removed lines as fit, then added
    let removed_budget = budget.min(preview.removed.len());
    let added_budget = budget
        .saturating_sub(removed_budget)
        .min(preview.added.len());

    let mut lines = Vec::with_capacity(budget + usize::from(truncated));

    let removed_style = DiffStyle {
        prefix: "  - ",
        fg: theme.diff.removed_fg,
        bg: theme.diff.removed_bg,
    };
    render_diff_section(
        &mut lines,
        &preview.removed,
        removed_budget,
        syntax,
        &removed_style,
        theme,
    );

    let added_style = DiffStyle {
        prefix: "  + ",
        fg: theme.diff.added_fg,
        bg: theme.diff.added_bg,
    };
    render_diff_section(
        &mut lines,
        &preview.added,
        added_budget,
        syntax,
        &added_style,
        theme,
    );

    if truncated {
        let remaining = total - budget;
        let mut overflow = Line::default();
        overflow.push_styled(format!("    ... {remaining} more lines"), theme.muted);
        lines.push(overflow);
    }

    lines
}

fn render_diff_section(
    lines: &mut Vec<Line>,
    source_lines: &[String],
    limit: usize,
    syntax: Option<&'static SyntaxReference>,
    style: &DiffStyle<'_>,
    theme: &Theme,
) {
    let mut highlighter = syntax.map(|s| HighlightLines::new(s, &SYNTECT.theme));
    for src in source_lines.iter().take(limit) {
        let mut line = Line::default();
        line.push_span(Span::with_style(
            style.prefix,
            Style::fg(style.fg).bg_color(style.bg),
        ));
        push_highlighted_spans(&mut line, src, &mut highlighter, style.bg, theme);
        lines.push(line);
    }
}

fn push_highlighted_spans(
    line: &mut Line,
    src: &str,
    highlighter: &mut Option<HighlightLines<'_>>,
    bg: Color,
    theme: &Theme,
) {
    let st = &*SYNTECT;

    if let Some(h) = highlighter
        && let Ok(ranges) = h.highlight_line(src, &st.syntax_set)
    {
        for (syntect_style, text) in ranges {
            let mut style = syntect_to_wisp_style(syntect_style);
            style.bg = Some(bg);
            line.push_span(Span::with_style(text, style));
        }
        return;
    }

    // Fallback: plain text with bg tint
    line.push_span(Span::with_style(src, Style::fg(theme.code_fg).bg_color(bg)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn removed_lines_have_minus_prefix() {
        let preview = DiffPreview {
            removed: vec!["old line".to_string()],
            added: vec![],
            lang_hint: String::new(),
        };
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("- old line"));
    }

    #[test]
    fn added_lines_have_plus_prefix() {
        let preview = DiffPreview {
            removed: vec![],
            added: vec!["new line".to_string()],
            lang_hint: String::new(),
        };
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("+ new line"));
    }

    #[test]
    fn both_removed_and_added() {
        let preview = DiffPreview {
            removed: vec!["old".to_string()],
            added: vec!["new".to_string()],
            lang_hint: String::new(),
        };
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("- old"));
        assert!(lines[1].plain_text().contains("+ new"));
    }

    #[test]
    fn truncates_long_diffs() {
        let preview = DiffPreview {
            removed: (0..15).map(|i| format!("removed {i}")).collect(),
            added: (0..15).map(|i| format!("added {i}")).collect(),
            lang_hint: String::new(),
        };
        let lines = highlight_diff(&preview, &test_theme());
        // 20 content lines + 1 overflow line
        assert_eq!(lines.len(), MAX_DIFF_LINES + 1);
        let last = lines.last().unwrap().plain_text();
        assert!(
            last.contains("more lines"),
            "Expected overflow text: {last}"
        );
    }

    #[test]
    fn no_truncation_at_boundary() {
        let preview = DiffPreview {
            removed: (0..10).map(|i| format!("r{i}")).collect(),
            added: (0..10).map(|i| format!("a{i}")).collect(),
            lang_hint: String::new(),
        };
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 20);
        assert!(!lines.last().unwrap().plain_text().contains("more lines"));
    }

    #[test]
    fn syntax_highlighting_with_known_lang() {
        let preview = DiffPreview {
            removed: vec!["fn old() {}".to_string()],
            added: vec!["fn new() {}".to_string()],
            lang_hint: "rs".to_string(),
        };
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 2);
        // With syntax highlighting, there should be multiple spans (not just 2: prefix + text)
        assert!(
            lines[0].spans().len() > 2,
            "Expected syntax-highlighted spans, got {} spans",
            lines[0].spans().len()
        );
    }

    #[test]
    fn removed_lines_have_red_bg() {
        let preview = DiffPreview {
            removed: vec!["old".to_string()],
            added: vec![],
            lang_hint: String::new(),
        };
        let theme = test_theme();
        let lines = highlight_diff(&preview, &theme);
        let prefix_span = &lines[0].spans()[0];
        assert_eq!(prefix_span.style().bg, Some(theme.diff.removed_bg));
        assert_eq!(prefix_span.style().fg, Some(theme.diff.removed_fg));
    }

    #[test]
    fn added_lines_have_green_bg() {
        let preview = DiffPreview {
            removed: vec![],
            added: vec!["new".to_string()],
            lang_hint: String::new(),
        };
        let theme = test_theme();
        let lines = highlight_diff(&preview, &theme);
        let prefix_span = &lines[0].spans()[0];
        assert_eq!(prefix_span.style().bg, Some(theme.diff.added_bg));
        assert_eq!(prefix_span.style().fg, Some(theme.diff.added_fg));
    }

    #[test]
    fn diff_uses_theme_palette_values() {
        let mut theme = test_theme();
        theme.diff.removed_bg = Color::Blue;
        theme.diff.removed_fg = Color::White;

        let preview = DiffPreview {
            removed: vec!["old".to_string()],
            added: vec![],
            lang_hint: String::new(),
        };
        let lines = highlight_diff(&preview, &theme);
        let prefix_span = &lines[0].spans()[0];
        assert_eq!(prefix_span.style().bg, Some(Color::Blue));
        assert_eq!(prefix_span.style().fg, Some(Color::White));
    }

    #[test]
    fn empty_diff_produces_no_lines() {
        let preview = DiffPreview {
            removed: vec![],
            added: vec![],
            lang_hint: String::new(),
        };
        let lines = highlight_diff(&preview, &test_theme());
        assert!(lines.is_empty());
    }
}
