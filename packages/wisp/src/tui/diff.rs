use crossterm::style::Color;
use syntect::easy::HighlightLines;

use acp_utils::notifications::{DiffPreview, DiffTag};

use super::screen::{Line, Span, Style};
use super::syntax::{find_syntax_for_hint, syntax_set, syntect_to_wisp_style};
use super::theme::Theme;

const MAX_DIFF_LINES: usize = 20;

struct DiffStyle<'a> {
    prefix: &'a str,
    fg: Color,
    bg: Color,
}

/// Render a diff preview with syntax-highlighted context/removed/added lines.
///
/// Context lines are shown with a `"    "` prefix and code background.
/// Removed lines are shown with a `"  - "` prefix and red-tinted background.
/// Added lines are shown with a `"  + "` prefix and green-tinted background.
pub fn highlight_diff(preview: &DiffPreview, theme: &Theme) -> Vec<Line> {
    let syntax = find_syntax_for_hint(&preview.lang_hint);

    let total = preview.lines.len();
    let truncated = total > MAX_DIFF_LINES;
    let budget = if truncated { MAX_DIFF_LINES } else { total };

    let mut lines = Vec::with_capacity(budget + usize::from(truncated));

    let syntect_theme = theme.syntect_theme();
    let mut highlighter = syntax.map(|s| HighlightLines::new(s, syntect_theme));

    let context_style = DiffStyle {
        prefix: "    ",
        fg: theme.code_fg(),
        bg: theme.code_bg(),
    };
    let removed_style = DiffStyle {
        prefix: "  - ",
        fg: theme.diff_removed_fg(),
        bg: theme.diff_removed_bg(),
    };
    let added_style = DiffStyle {
        prefix: "  + ",
        fg: theme.diff_added_fg(),
        bg: theme.diff_added_bg(),
    };

    let mut old_line = preview.start_line.unwrap_or(0);

    for diff_line in preview.lines.iter().take(budget) {
        let style = match diff_line.tag {
            DiffTag::Context => &context_style,
            DiffTag::Removed => &removed_style,
            DiffTag::Added => &added_style,
        };

        let mut line = Line::default();

        if preview.start_line.is_some() {
            match diff_line.tag {
                DiffTag::Context | DiffTag::Removed => {
                    let line_num = format!("{old_line:>4} ");
                    line.push_styled(line_num, theme.muted());
                }
                DiffTag::Added => {
                    line.push_styled("     ", theme.muted());
                }
            }
        }

        line.push_span(Span::with_style(
            style.prefix,
            Style::fg(style.fg).bg_color(style.bg),
        ));
        push_highlighted_spans(
            &mut line,
            &diff_line.content,
            &mut highlighter,
            style.bg,
            theme,
        );
        lines.push(line);

        if matches!(diff_line.tag, DiffTag::Context | DiffTag::Removed) {
            old_line += 1;
        }
    }

    if truncated {
        let remaining = total - budget;
        let mut overflow = Line::default();
        overflow.push_styled(format!("    ... {remaining} more lines"), theme.muted());
        lines.push(overflow);
    }

    lines
}

fn push_highlighted_spans(
    line: &mut Line,
    src: &str,
    highlighter: &mut Option<HighlightLines<'_>>,
    bg: Color,
    theme: &Theme,
) {
    if let Some(h) = highlighter
        && let Ok(ranges) = h.highlight_line(src, syntax_set())
    {
        for (syntect_style, text) in ranges {
            let mut style = syntect_to_wisp_style(syntect_style);
            style.bg = Some(bg);
            line.push_span(Span::with_style(text, style));
        }
        return;
    }

    // Fallback: plain text with bg tint
    line.push_span(Span::with_style(
        src,
        Style::fg(theme.code_fg()).bg_color(bg),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_utils::notifications::DiffLine;

    fn test_theme() -> Theme {
        Theme::default()
    }

    fn make_preview(lines: Vec<DiffLine>) -> DiffPreview {
        DiffPreview {
            lines,
            lang_hint: String::new(),
            start_line: None,
        }
    }

    #[test]
    fn removed_lines_have_minus_prefix() {
        let preview = make_preview(vec![DiffLine {
            tag: DiffTag::Removed,
            content: "old line".to_string(),
        }]);
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("- old line"));
    }

    #[test]
    fn added_lines_have_plus_prefix() {
        let preview = make_preview(vec![DiffLine {
            tag: DiffTag::Added,
            content: "new line".to_string(),
        }]);
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("+ new line"));
    }

    #[test]
    fn context_lines_have_no_diff_prefix() {
        let preview = make_preview(vec![DiffLine {
            tag: DiffTag::Context,
            content: "unchanged".to_string(),
        }]);
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(
            text.starts_with("    "),
            "context should have space prefix: {text}"
        );
        assert!(!text.contains("+ "), "context should not have + prefix");
        assert!(!text.contains("- "), "context should not have - prefix");
    }

    #[test]
    fn mixed_diff_renders_correctly() {
        let preview = make_preview(vec![
            DiffLine {
                tag: DiffTag::Context,
                content: "before".to_string(),
            },
            DiffLine {
                tag: DiffTag::Removed,
                content: "old".to_string(),
            },
            DiffLine {
                tag: DiffTag::Added,
                content: "new".to_string(),
            },
            DiffLine {
                tag: DiffTag::Context,
                content: "after".to_string(),
            },
        ]);
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 4);
        assert!(lines[0].plain_text().contains("before"));
        assert!(lines[1].plain_text().contains("- old"));
        assert!(lines[2].plain_text().contains("+ new"));
        assert!(lines[3].plain_text().contains("after"));
    }

    #[test]
    fn both_removed_and_added() {
        let preview = make_preview(vec![
            DiffLine {
                tag: DiffTag::Removed,
                content: "old".to_string(),
            },
            DiffLine {
                tag: DiffTag::Added,
                content: "new".to_string(),
            },
        ]);
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("- old"));
        assert!(lines[1].plain_text().contains("+ new"));
    }

    #[test]
    fn truncates_long_diffs() {
        let diff_lines: Vec<DiffLine> = (0..30)
            .map(|i| DiffLine {
                tag: if i % 2 == 0 {
                    DiffTag::Removed
                } else {
                    DiffTag::Added
                },
                content: format!("line {i}"),
            })
            .collect();
        let preview = make_preview(diff_lines);
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
        let diff_lines: Vec<DiffLine> = (0..20)
            .map(|i| DiffLine {
                tag: if i % 2 == 0 {
                    DiffTag::Removed
                } else {
                    DiffTag::Added
                },
                content: format!("line {i}"),
            })
            .collect();
        let preview = make_preview(diff_lines);
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 20);
        assert!(!lines.last().unwrap().plain_text().contains("more lines"));
    }

    #[test]
    fn syntax_highlighting_with_known_lang() {
        let preview = DiffPreview {
            lines: vec![
                DiffLine {
                    tag: DiffTag::Removed,
                    content: "fn old() {}".to_string(),
                },
                DiffLine {
                    tag: DiffTag::Added,
                    content: "fn new() {}".to_string(),
                },
            ],
            lang_hint: "rs".to_string(),
            start_line: None,
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
        let preview = make_preview(vec![DiffLine {
            tag: DiffTag::Removed,
            content: "old".to_string(),
        }]);
        let theme = test_theme();
        let lines = highlight_diff(&preview, &theme);
        let prefix_span = &lines[0].spans()[0];
        assert_eq!(prefix_span.style().bg, Some(theme.diff_removed_bg()));
        assert_eq!(prefix_span.style().fg, Some(theme.diff_removed_fg()));
    }

    #[test]
    fn added_lines_have_green_bg() {
        let preview = make_preview(vec![DiffLine {
            tag: DiffTag::Added,
            content: "new".to_string(),
        }]);
        let theme = test_theme();
        let lines = highlight_diff(&preview, &theme);
        let prefix_span = &lines[0].spans()[0];
        assert_eq!(prefix_span.style().bg, Some(theme.diff_added_bg()));
        assert_eq!(prefix_span.style().fg, Some(theme.diff_added_fg()));
    }

    #[test]
    fn context_lines_have_code_bg() {
        let preview = make_preview(vec![DiffLine {
            tag: DiffTag::Context,
            content: "same".to_string(),
        }]);
        let theme = test_theme();
        let lines = highlight_diff(&preview, &theme);
        let prefix_span = &lines[0].spans()[0];
        assert_eq!(prefix_span.style().bg, Some(theme.code_bg()));
    }

    #[test]
    fn empty_diff_produces_no_lines() {
        let preview = make_preview(vec![]);
        let lines = highlight_diff(&preview, &test_theme());
        assert!(lines.is_empty());
    }

    #[test]
    fn line_numbers_rendered_when_start_line_set() {
        let preview = DiffPreview {
            lines: vec![
                DiffLine {
                    tag: DiffTag::Context,
                    content: "ctx".to_string(),
                },
                DiffLine {
                    tag: DiffTag::Removed,
                    content: "old".to_string(),
                },
                DiffLine {
                    tag: DiffTag::Added,
                    content: "new".to_string(),
                },
                DiffLine {
                    tag: DiffTag::Context,
                    content: "ctx2".to_string(),
                },
            ],
            lang_hint: String::new(),
            start_line: Some(10),
        };
        let lines = highlight_diff(&preview, &test_theme());
        assert_eq!(lines.len(), 4);
        assert!(
            lines[0].plain_text().contains("10"),
            "context line should show 10"
        );
        assert!(
            lines[1].plain_text().contains("11"),
            "removed line should show 11"
        );
        // Added lines don't show a source line number
        let added_text = lines[2].plain_text();
        assert!(
            !added_text.starts_with("  12"),
            "added line should not show line number"
        );
        assert!(
            lines[3].plain_text().contains("12"),
            "next context should show 12"
        );
    }

    #[test]
    fn no_line_numbers_when_start_line_none() {
        let preview = make_preview(vec![DiffLine {
            tag: DiffTag::Removed,
            content: "old".to_string(),
        }]);
        let lines = highlight_diff(&preview, &test_theme());
        let text = lines[0].plain_text();
        // Without line numbers, the line should start with the prefix directly
        assert!(
            text.starts_with("  - "),
            "expected prefix without line number gutter: {text}"
        );
    }
}
