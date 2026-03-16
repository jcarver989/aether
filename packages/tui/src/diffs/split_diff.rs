use crate::diffs::diff::highlight_diff;
use crate::line::Line;
use crate::rendering::render_context::ViewContext;
use crate::rendering::soft_wrap::truncate_line;
use crate::span::Span;
use crate::style::Style;
use crate::{DiffPreview, DiffTag, SplitDiffCell};

const MAX_DIFF_LINES: usize = 20;
const MIN_SPLIT_WIDTH: u16 = 80;
const GUTTER_WIDTH: usize = 5;
const SEPARATOR: &str = " \u{2502} ";
const SEPARATOR_WIDTH: usize = 3;
const FIXED_OVERHEAD: usize = GUTTER_WIDTH * 2 + SEPARATOR_WIDTH;

/// Renders a diff preview, choosing split or unified based on terminal width.
pub fn render_diff(preview: &DiffPreview, context: &ViewContext) -> Vec<Line> {
    if context.size.width >= MIN_SPLIT_WIDTH {
        highlight_split_diff(preview, context)
    } else {
        highlight_diff(preview, context)
    }
}

fn highlight_split_diff(preview: &DiffPreview, context: &ViewContext) -> Vec<Line> {
    let theme = &context.theme;
    let total = preview.rows.len();
    let truncated = total > MAX_DIFF_LINES;
    let budget = if truncated { MAX_DIFF_LINES } else { total };

    let terminal_width = context.size.width as usize;
    let usable = terminal_width.saturating_sub(FIXED_OVERHEAD);
    let left_content = usable / 2;
    let right_content = usable - left_content;

    let mut lines = Vec::with_capacity(budget + usize::from(truncated));

    for row in preview.rows.iter().take(budget) {
        let mut line = Line::default();

        render_cell(
            &mut line,
            row.left.as_ref(),
            left_content,
            &preview.lang_hint,
            context,
        );

        line.push_styled(SEPARATOR, theme.muted());

        render_cell(
            &mut line,
            row.right.as_ref(),
            right_content,
            &preview.lang_hint,
            context,
        );

        lines.push(line);
    }

    if truncated {
        let remaining = total - budget;
        let mut overflow = Line::default();
        overflow.push_styled(format!("    ... {remaining} more lines"), theme.muted());
        lines.push(overflow);
    }

    lines
}

fn render_cell(
    line: &mut Line,
    cell: Option<&SplitDiffCell>,
    content_width: usize,
    lang_hint: &str,
    context: &ViewContext,
) {
    let theme = &context.theme;
    let panel_width = GUTTER_WIDTH + content_width;

    let Some(cell) = cell else {
        line.push_text(" ".repeat(panel_width));
        return;
    };

    let bg = match cell.tag {
        DiffTag::Removed => Some(theme.diff_removed_bg()),
        DiffTag::Added => Some(theme.diff_added_bg()),
        DiffTag::Context => None,
    };

    // Gutter (5 cols)
    if let Some(num) = cell.line_number {
        line.push_styled(format!("{num:>4} "), theme.muted());
    } else {
        line.push_styled("     ", theme.muted());
    }

    // Syntax-highlighted content
    let highlighted = context
        .highlighter()
        .highlight(&cell.content, lang_hint, theme);

    let content_line = if let Some(hl_line) = highlighted.first() {
        let mut styled_content = Line::default();
        for span in hl_line.spans() {
            let mut span_style = span.style();
            if let Some(bg) = bg {
                span_style.bg = Some(bg);
            }
            styled_content.push_span(Span::with_style(span.text(), span_style));
        }
        styled_content
    } else {
        let fg = match cell.tag {
            DiffTag::Removed => theme.diff_removed_fg(),
            DiffTag::Added => theme.diff_added_fg(),
            DiffTag::Context => theme.code_fg(),
        };
        let mut style = Style::fg(fg);
        if let Some(bg) = bg {
            style = style.bg_color(bg);
        }
        Line::with_style(&cell.content, style)
    };

    let mut truncated_content = truncate_line(&content_line, content_width);
    truncated_content.extend_bg_to_width(content_width);
    line.append_line(&truncated_content);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiffLine, SplitDiffCell, SplitDiffRow};

    fn test_context_with_width(width: u16) -> ViewContext {
        ViewContext::new((width, 24))
    }

    fn make_split_preview(rows: Vec<SplitDiffRow>) -> DiffPreview {
        DiffPreview {
            lines: vec![],
            rows,
            lang_hint: String::new(),
            start_line: None,
        }
    }

    fn context_cell(content: &str, line_num: usize) -> SplitDiffCell {
        SplitDiffCell {
            tag: DiffTag::Context,
            content: content.to_string(),
            line_number: Some(line_num),
        }
    }

    fn removed_cell(content: &str, line_num: usize) -> SplitDiffCell {
        SplitDiffCell {
            tag: DiffTag::Removed,
            content: content.to_string(),
            line_number: Some(line_num),
        }
    }

    fn added_cell(content: &str, line_num: usize) -> SplitDiffCell {
        SplitDiffCell {
            tag: DiffTag::Added,
            content: content.to_string(),
            line_number: Some(line_num),
        }
    }

    #[test]
    fn both_panels_rendered_with_content() {
        let preview = make_split_preview(vec![SplitDiffRow {
            left: Some(removed_cell("old code", 1)),
            right: Some(added_cell("new code", 1)),
        }]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("old code"), "left panel missing: {text}");
        assert!(text.contains("new code"), "right panel missing: {text}");
    }

    #[test]
    fn separator_present_in_each_line() {
        let preview = make_split_preview(vec![
            SplitDiffRow {
                left: Some(context_cell("aaa", 1)),
                right: Some(context_cell("aaa", 1)),
            },
            SplitDiffRow {
                left: Some(removed_cell("bbb", 2)),
                right: Some(added_cell("BBB", 2)),
            },
        ]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        for line in &lines {
            let text = line.plain_text();
            assert!(
                text.contains('\u{2502}'),
                "separator missing in line: {text}"
            );
        }
    }

    #[test]
    fn long_lines_truncated_within_terminal_width() {
        let long = "x".repeat(200);
        let preview = make_split_preview(vec![SplitDiffRow {
            left: Some(removed_cell(&long, 1)),
            right: Some(added_cell(&long, 1)),
        }]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        assert_eq!(lines.len(), 1);
        let width = lines[0].display_width();
        assert!(
            width <= 100,
            "line width {width} should not exceed terminal width 100"
        );
    }

    #[test]
    fn truncation_budget_applied() {
        let rows: Vec<SplitDiffRow> = (0..30)
            .map(|i| SplitDiffRow {
                left: Some(removed_cell(&format!("old {i}"), i + 1)),
                right: Some(added_cell(&format!("new {i}"), i + 1)),
            })
            .collect();
        let preview = make_split_preview(rows);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        // 20 content rows + 1 overflow
        assert_eq!(lines.len(), MAX_DIFF_LINES + 1);
        let last = lines.last().unwrap().plain_text();
        assert!(last.contains("more lines"), "overflow text missing: {last}");
    }

    #[test]
    fn empty_preview_produces_no_output() {
        let preview = make_split_preview(vec![]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        assert!(lines.is_empty());
    }

    #[test]
    fn render_diff_dispatches_to_unified_below_80() {
        let preview = DiffPreview {
            lines: vec![DiffLine {
                tag: DiffTag::Removed,
                content: "old".to_string(),
            }],
            rows: vec![SplitDiffRow {
                left: Some(removed_cell("old", 1)),
                right: None,
            }],
            lang_hint: String::new(),
            start_line: None,
        };
        let ctx = test_context_with_width(79);
        let lines = render_diff(&preview, &ctx);
        // Unified renderer uses prefix "  - "
        assert!(
            lines[0].plain_text().contains("- old"),
            "should use unified renderer below 80: {}",
            lines[0].plain_text()
        );
    }

    #[test]
    fn render_diff_dispatches_to_split_at_80() {
        let preview = DiffPreview {
            lines: vec![DiffLine {
                tag: DiffTag::Removed,
                content: "old".to_string(),
            }],
            rows: vec![SplitDiffRow {
                left: Some(removed_cell("old", 1)),
                right: None,
            }],
            lang_hint: String::new(),
            start_line: None,
        };
        let ctx = test_context_with_width(80);
        let lines = render_diff(&preview, &ctx);
        let text = lines[0].plain_text();
        assert!(
            text.contains('\u{2502}'),
            "should use split renderer at 80: {text}"
        );
    }

    #[test]
    fn line_numbers_rendered_when_start_line_set() {
        let preview = make_split_preview(vec![SplitDiffRow {
            left: Some(SplitDiffCell {
                tag: DiffTag::Context,
                content: "hello".to_string(),
                line_number: Some(42),
            }),
            right: Some(SplitDiffCell {
                tag: DiffTag::Context,
                content: "hello".to_string(),
                line_number: Some(42),
            }),
        }]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        let text = lines[0].plain_text();
        assert!(text.contains("42"), "line number should be shown: {text}");
    }

    #[test]
    fn blank_gutter_when_line_number_none() {
        let preview = make_split_preview(vec![SplitDiffRow {
            left: Some(SplitDiffCell {
                tag: DiffTag::Removed,
                content: "old".to_string(),
                line_number: None,
            }),
            right: None,
        }]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        let text = lines[0].plain_text();
        assert!(
            text.starts_with("     "),
            "should have blank gutter: {text:?}"
        );
    }
}
