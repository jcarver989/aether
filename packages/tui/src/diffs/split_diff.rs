use crate::diffs::diff::highlight_diff;
use crate::line::Line;
use crate::rendering::frame::{FitOptions, Frame, FramePart};
use crate::rendering::render_context::ViewContext;
use crate::span::Span;
use crate::style::Style;

use crate::{DiffPreview, DiffTag, SplitDiffCell};

const MAX_DIFF_LINES: usize = 20;
pub const MIN_SPLIT_WIDTH: u16 = 80;
pub const GUTTER_WIDTH: usize = 5;
pub const SEPARATOR: &str = "   ";
pub const SEPARATOR_WIDTH: usize = 3;
const SEPARATOR_WIDTH_U16: u16 = 3;
const FIXED_OVERHEAD: usize = GUTTER_WIDTH * 2 + SEPARATOR_WIDTH;

/// Renders a diff preview, choosing split or unified based on terminal width
/// and whether the diff has removals.
///
/// For new files (only additions, no removals), uses unified view since
/// split view would have an empty left panel.
pub fn render_diff(preview: &DiffPreview, context: &ViewContext) -> Vec<Line> {
    let has_removals = preview.lines.iter().any(|l| l.tag == DiffTag::Removed);

    if context.size.width >= MIN_SPLIT_WIDTH && has_removals {
        highlight_split_diff(preview, context)
    } else {
        highlight_diff(preview, context)
    }
}

fn highlight_split_diff(preview: &DiffPreview, context: &ViewContext) -> Vec<Line> {
    let theme = &context.theme;
    let terminal_width = context.size.width as usize;
    let usable = terminal_width.saturating_sub(FIXED_OVERHEAD);
    let left_content = usable / 2;
    let right_content = usable - left_content;
    #[allow(clippy::cast_possible_truncation)]
    let left_panel_u16 = (GUTTER_WIDTH + left_content) as u16;
    #[allow(clippy::cast_possible_truncation)]
    let right_panel_u16 = (GUTTER_WIDTH + right_content) as u16;
    let sep_style = Style::fg(theme.muted()).bg_color(theme.background());

    let mut row_frames: Vec<Frame> = Vec::new();
    let mut visual_lines = 0usize;
    let mut rows_consumed = 0usize;

    for row in &preview.rows {
        let left_frame = render_cell(row.left.as_ref(), left_content, &preview.lang_hint, context);
        let right_frame = render_cell(row.right.as_ref(), right_content, &preview.lang_hint, context);

        let height = left_frame.lines().len().max(right_frame.lines().len());

        if visual_lines + height > MAX_DIFF_LINES && visual_lines > 0 {
            break;
        }

        let sep_line = Line::with_style(SEPARATOR.to_string(), sep_style);
        let sep_frame = Frame::new(vec![sep_line; height]);
        row_frames.push(Frame::hstack([
            FramePart::new(left_frame, left_panel_u16),
            FramePart::new(sep_frame, SEPARATOR_WIDTH_U16),
            FramePart::new(right_frame, right_panel_u16),
        ]));

        visual_lines += height;
        rows_consumed += 1;
    }

    let mut lines = Frame::vstack(row_frames).into_lines();

    if rows_consumed < preview.rows.len() {
        let remaining = preview.rows.len() - rows_consumed;
        let mut overflow = Line::default();
        overflow.push_styled(format!("    ... {remaining} more lines"), theme.muted());
        lines.push(overflow);
    }

    lines
}

fn blank_panel(width: usize) -> Line {
    let mut line = Line::default();
    line.push_text(" ".repeat(width));
    line
}

pub fn render_cell(
    cell: Option<&SplitDiffCell>,
    content_width: usize,
    lang_hint: &str,
    context: &ViewContext,
) -> Frame {
    let theme = &context.theme;
    let panel_width = GUTTER_WIDTH + content_width;

    let Some(cell) = cell else {
        return Frame::new(vec![blank_panel(panel_width)]);
    };

    let is_context = cell.tag == DiffTag::Context;
    let bg = match cell.tag {
        DiffTag::Removed => Some(theme.diff_removed_bg()),
        DiffTag::Added => Some(theme.diff_added_bg()),
        DiffTag::Context => None,
    };

    // Syntax-highlighted content
    let highlighted = context.highlighter().highlight(&cell.content, lang_hint, theme);

    let content_line = if let Some(hl_line) = highlighted.first() {
        let mut styled_content = Line::default();
        for span in hl_line.spans() {
            let mut span_style = span.style();
            if let Some(bg) = bg {
                span_style.bg = Some(bg);
            }
            if is_context {
                span_style.dim = true;
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
        if is_context {
            style.dim = true;
        }
        Line::with_style(&cell.content, style)
    };

    // content_width is derived from terminal width (u16), so it always fits in u16
    #[allow(clippy::cast_possible_truncation)]
    let content_width_u16 = content_width as u16;

    let gutter_style = Style::fg(theme.muted());
    let head = match cell.line_number {
        Some(num) => Line::with_style(format!("{num:>4} "), gutter_style),
        None => Line::with_style("     ".to_string(), gutter_style),
    };
    let tail = Line::new(" ".repeat(GUTTER_WIDTH));

    Frame::new(vec![content_line])
        .fit(content_width_u16, FitOptions::wrap())
        .map_lines(|mut line| {
            line.extend_bg_to_width(content_width);
            line
        })
        .prefix(&head, &tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::line::Line;
    use crate::{DiffLine, SplitDiffCell, SplitDiffRow};

    fn test_context_with_width(width: u16) -> ViewContext {
        ViewContext::new((width, 24))
    }

    fn make_split_preview(rows: Vec<SplitDiffRow>) -> DiffPreview {
        DiffPreview { lines: vec![], rows, lang_hint: String::new(), start_line: None }
    }

    fn removed_cell(content: &str, line_num: usize) -> SplitDiffCell {
        SplitDiffCell { tag: DiffTag::Removed, content: content.to_string(), line_number: Some(line_num) }
    }

    fn added_cell(content: &str, line_num: usize) -> SplitDiffCell {
        SplitDiffCell { tag: DiffTag::Added, content: content.to_string(), line_number: Some(line_num) }
    }

    fn style_at_column(line: &Line, col: usize) -> Style {
        let mut current = 0;
        for span in line.spans() {
            let width = crate::display_width_text(span.text());
            if col < current + width {
                return span.style();
            }
            current += width;
        }
        Style::default()
    }

    #[test]
    fn wrapped_split_rows_preserve_neutral_boundary_columns() {
        let preview = make_split_preview(vec![SplitDiffRow {
            left: Some(removed_cell("LEFT_MARK", 1)),
            right: Some(added_cell(&format!("RIGHT_HEAD {} RIGHT_TAIL", "y".repeat(140)), 1)),
        }]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);

        let first_row = lines
            .iter()
            .position(|line| {
                let text = line.plain_text();
                text.contains("LEFT_MARK") && text.contains("RIGHT_HEAD")
            })
            .expect("expected split row containing both left and right markers");

        let right_start =
            lines[first_row].plain_text().find("RIGHT_HEAD").expect("expected RIGHT_HEAD marker in first split row");

        let wrapped_row = lines
            .iter()
            .enumerate()
            .skip(first_row + 1)
            .find_map(|(index, line)| line.plain_text().contains("RIGHT_TAIL").then_some(index))
            .expect("expected wrapped continuation row containing RIGHT_TAIL marker");

        let wrapped_start = lines[wrapped_row]
            .plain_text()
            .find("RIGHT_TAIL")
            .expect("expected RIGHT_TAIL marker in wrapped continuation row");

        assert!(
            wrapped_start >= right_start,
            "wrapped continuation should not start left of original right-pane content start (was {wrapped_start}, expected >= {right_start})"
        );

        let added_bg = ctx.theme.diff_added_bg();
        let removed_bg = ctx.theme.diff_removed_bg();
        let padding_width = GUTTER_WIDTH + SEPARATOR_WIDTH;
        assert!(right_start >= padding_width, "right pane content should leave room for separator and gutter");
        for col in (right_start - padding_width)..right_start {
            let style = style_at_column(&lines[wrapped_row], col);
            assert_ne!(style.bg, Some(added_bg), "padding column {col} should not inherit added background");
            assert_ne!(style.bg, Some(removed_bg), "padding column {col} should not inherit removed background");
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
    fn long_lines_wrapped_within_terminal_width() {
        let long = "x".repeat(200);
        let preview = make_split_preview(vec![SplitDiffRow {
            left: Some(removed_cell(&long, 1)),
            right: Some(added_cell(&long, 1)),
        }]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        assert!(lines.len() > 1, "long line should wrap into multiple visual lines, got {}", lines.len());
        for line in &lines {
            let width = line.display_width();
            assert!(width <= 100, "line width {width} should not exceed terminal width 100");
        }
        // Full content should be present across all wrapped lines
        let all_text: String = lines.iter().map(Line::plain_text).collect();
        let x_count = all_text.chars().filter(|&c| c == 'x').count();
        // Both left and right panels contain 200 x's each
        assert_eq!(x_count, 400, "all content should be present across wrapped lines");
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
        // Short lines don't wrap, so 20 visual lines + 1 overflow
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
            lines: vec![DiffLine { tag: DiffTag::Removed, content: "old".to_string() }],
            rows: vec![SplitDiffRow { left: Some(removed_cell("old", 1)), right: None }],
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
    fn new_file_uses_unified_view_even_at_wide_width() {
        // A new file (only additions, no removals) should use unified view
        // since split view would have an empty left panel
        let preview = DiffPreview {
            lines: vec![
                DiffLine { tag: DiffTag::Added, content: "fn main() {".to_string() },
                DiffLine { tag: DiffTag::Added, content: "    println!(\"Hello\");".to_string() },
                DiffLine { tag: DiffTag::Added, content: "}".to_string() },
            ],
            rows: vec![
                SplitDiffRow { left: None, right: Some(added_cell("fn main() {", 1)) },
                SplitDiffRow { left: None, right: Some(added_cell("    println!(\"Hello\");", 2)) },
                SplitDiffRow { left: None, right: Some(added_cell("}", 3)) },
            ],
            lang_hint: "rs".to_string(),
            start_line: None,
        };
        let ctx = test_context_with_width(100);
        let lines = render_diff(&preview, &ctx);
        // Unified renderer uses prefix "  + "
        let text = lines[0].plain_text();
        assert!(text.contains("+ fn main()"), "should use unified renderer for new file: {text}");
    }

    #[test]
    fn render_diff_dispatches_to_split_at_80() {
        let preview = DiffPreview {
            lines: vec![DiffLine { tag: DiffTag::Removed, content: "old".to_string() }],
            rows: vec![SplitDiffRow { left: Some(removed_cell("old", 1)), right: None }],
            lang_hint: String::new(),
            start_line: None,
        };
        let ctx = test_context_with_width(80);
        let lines = render_diff(&preview, &ctx);
        let text = lines[0].plain_text();
        // Split renderer shows line number gutter, not unified "- " prefix
        assert!(!text.contains("- old"), "should use split renderer at 80: {text}");
    }

    #[test]
    fn line_numbers_rendered_when_start_line_set() {
        let preview = make_split_preview(vec![SplitDiffRow {
            left: Some(SplitDiffCell { tag: DiffTag::Context, content: "hello".to_string(), line_number: Some(42) }),
            right: Some(SplitDiffCell { tag: DiffTag::Context, content: "hello".to_string(), line_number: Some(42) }),
        }]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        let text = lines[0].plain_text();
        assert!(text.contains("42"), "line number should be shown: {text}");
    }

    #[test]
    fn wrapped_row_pads_shorter_side_to_match_height() {
        // Left side has a long line that wraps, right side is short
        let long = "a".repeat(200);
        let preview = make_split_preview(vec![SplitDiffRow {
            left: Some(removed_cell(&long, 1)),
            right: Some(added_cell("short", 1)),
        }]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        assert!(lines.len() > 1, "long left side should produce multiple visual lines");
        // All lines should have consistent width
        let first_width = lines[0].display_width();
        for (i, line) in lines.iter().enumerate() {
            assert_eq!(line.display_width(), first_width, "line {i} width mismatch");
        }
    }

    #[test]
    fn blank_gutter_when_line_number_none() {
        let preview = make_split_preview(vec![SplitDiffRow {
            left: Some(SplitDiffCell { tag: DiffTag::Removed, content: "old".to_string(), line_number: None }),
            right: None,
        }]);
        let ctx = test_context_with_width(100);
        let lines = highlight_split_diff(&preview, &ctx);
        let text = lines[0].plain_text();
        assert!(text.starts_with("     "), "should have blank gutter: {text:?}");
    }
}
