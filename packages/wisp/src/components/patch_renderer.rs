use crate::components::app::git_diff_mode::{PatchLineRef, QueuedComment};
use crate::git_diff::{FileDiff, PatchLineKind};
use std::collections::HashMap;
use tui::{Color, FitOptions, Frame, Line, Span, Style, ViewContext};

pub(crate) fn build_comment_map(comments: &[QueuedComment]) -> HashMap<PatchLineRef, Vec<&QueuedComment>> {
    let mut map: HashMap<PatchLineRef, Vec<&QueuedComment>> = HashMap::new();
    for c in comments {
        map.entry(c.patch_ref).or_default().push(c);
    }
    map
}

pub fn build_patch_lines(
    file: &FileDiff,
    right_width: usize,
    context: &ViewContext,
    comments: &[QueuedComment],
) -> (Vec<Line>, Vec<Option<PatchLineRef>>) {
    let theme = &context.theme;
    let lang_hint = lang_hint_from_path(&file.path);
    let mut patch_lines = Vec::new();
    let mut patch_refs = Vec::new();

    let comment_map = build_comment_map(comments);

    let max_line_no = file
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter_map(|l| l.old_line_no.into_iter().chain(l.new_line_no).max())
        .max()
        .unwrap_or(0);
    let gutter_width = digit_count(max_line_no);

    for (hunk_idx, hunk) in file.hunks.iter().enumerate() {
        if hunk_idx > 0 {
            patch_lines.push(Line::default());
            patch_refs.push(None);
        }

        for (line_idx, pl) in hunk.lines.iter().enumerate() {
            let mut line = Line::default();

            match pl.kind {
                PatchLineKind::HunkHeader => {
                    line.push_with_style(&pl.text, Style::fg(theme.info()).bold().bg_color(theme.code_bg()));
                }
                PatchLineKind::Context => {
                    let old_str = format_line_no(pl.old_line_no, gutter_width);
                    let new_str = format_line_no(pl.new_line_no, gutter_width);
                    line.push_with_style(format!("{old_str} {new_str}   "), Style::fg(theme.text_secondary()));
                    append_syntax_spans(&mut line, &pl.text, lang_hint, None, context);
                }
                PatchLineKind::Added => {
                    let old_str = " ".repeat(gutter_width);
                    let new_str = format_line_no(pl.new_line_no, gutter_width);
                    let bg = Some(theme.diff_added_bg());
                    let style = Style::fg(theme.diff_added_fg()).bg_color(theme.diff_added_bg());
                    line.push_with_style(format!("{old_str} {new_str} + "), style);
                    append_syntax_spans(&mut line, &pl.text, lang_hint, bg, context);
                }
                PatchLineKind::Removed => {
                    let old_str = format_line_no(pl.old_line_no, gutter_width);
                    let new_str = " ".repeat(gutter_width);
                    let bg = Some(theme.diff_removed_bg());
                    let style = Style::fg(theme.diff_removed_fg()).bg_color(theme.diff_removed_bg());
                    line.push_with_style(format!("{old_str} {new_str} - "), style);
                    append_syntax_spans(&mut line, &pl.text, lang_hint, bg, context);
                }
                PatchLineKind::Meta => {
                    line.push_with_style(&pl.text, Style::fg(theme.text_secondary()).italic());
                }
            }

            let anchor = PatchLineRef { hunk_index: hunk_idx, line_index: line_idx };

            #[allow(clippy::cast_possible_truncation)]
            let right_width_u16 = right_width as u16;
            let wrapped = Frame::new(vec![line])
                .fit(right_width_u16, FitOptions::wrap())
                .map_lines(|mut l| {
                    l.extend_bg_to_width(right_width);
                    l
                })
                .into_lines();
            patch_refs.push(Some(anchor));
            patch_refs.extend(std::iter::repeat_n(None, wrapped.len().saturating_sub(1)));
            patch_lines.extend(wrapped);

            if let Some(line_comments) = comment_map.get(&anchor) {
                append_inline_comment_rows(&mut patch_lines, &mut patch_refs, line_comments, right_width, theme);
            }
        }
    }

    (patch_lines, patch_refs)
}

pub(crate) fn lang_hint_from_path(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or("")
}

pub(crate) fn append_syntax_spans(
    line: &mut Line,
    text: &str,
    lang_hint: &str,
    bg_override: Option<Color>,
    context: &ViewContext,
) {
    let spans = context.highlighter().highlight(text, lang_hint, &context.theme);
    if let Some(content) = spans.first() {
        for span in content.spans() {
            let mut span_style = span.style();
            if let Some(bg) = bg_override {
                span_style.bg = Some(bg);
            }
            line.push_span(Span::with_style(span.text(), span_style));
        }
    } else {
        line.push_text(text);
    }
}

pub(crate) fn append_inline_comment_rows(
    patch_lines: &mut Vec<Line>,
    patch_refs: &mut Vec<Option<PatchLineRef>>,
    comments: &[&QueuedComment],
    right_width: usize,
    theme: &tui::Theme,
) {
    let indent = 2;
    let box_left = "│ ";
    let bg = theme.sidebar_bg();
    let border_fg = theme.muted();
    let text_fg = theme.text_primary();

    let dashes = right_width.saturating_sub(indent + 1);

    for comment in comments {
        let inner_width = right_width.saturating_sub(indent + box_left.len() + 1);
        let wrapped = wrap_text(&comment.comment, inner_width);

        push_border_row(patch_lines, "┌", indent, dashes, right_width, border_fg, bg);
        patch_refs.push(None);

        for text_line in &wrapped {
            let mut row = Line::default();
            row.push_with_style(" ".repeat(indent), Style::default().bg_color(bg));
            row.push_with_style(box_left, Style::fg(border_fg).bg_color(bg));
            row.push_with_style(text_line.as_str(), Style::fg(text_fg).bg_color(bg));
            row.extend_bg_to_width(right_width);
            patch_lines.push(row);
            patch_refs.push(None);
        }

        push_border_row(patch_lines, "└", indent, dashes, right_width, border_fg, bg);
        patch_refs.push(None);
    }
}

fn push_border_row(
    lines: &mut Vec<Line>,
    corner: &str,
    indent: usize,
    dashes: usize,
    right_width: usize,
    border_fg: Color,
    bg: Color,
) {
    let mut row = Line::default();
    row.push_with_style(" ".repeat(indent), Style::default().bg_color(bg));
    row.push_with_style(corner, Style::fg(border_fg).bg_color(bg));
    row.push_with_style("─".repeat(dashes), Style::fg(border_fg).bg_color(bg));
    row.extend_bg_to_width(right_width);
    lines.push(row);
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if current_len == 0 {
            current.push_str(word);
            current_len = word_len;
        } else if current_len + 1 + word_len <= max_width {
            current.push(' ');
            current.push_str(word);
            current_len += 1 + word_len;
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
            current_len = word_len;
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

pub(crate) fn format_line_no(line_no: Option<usize>, width: usize) -> String {
    match line_no {
        Some(n) => format!("{n:>width$}"),
        None => " ".repeat(width),
    }
}

pub(crate) fn digit_count(mut n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    while n > 0 {
        count += 1;
        n /= 10;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_diff::{FileDiff, FileStatus, Hunk, PatchLine};
    use tui::display_width_text;

    fn make_file(lines: Vec<PatchLine>) -> FileDiff {
        FileDiff {
            old_path: Some("test.rs".to_string()),
            path: "test.rs".to_string(),
            status: FileStatus::Modified,
            hunks: vec![Hunk {
                header: "@@ -1,1 +1,1 @@".to_string(),
                old_start: 1,
                old_count: 1,
                new_start: 1,
                new_count: 1,
                lines,
            }],
            binary: false,
        }
    }

    #[test]
    fn long_lines_soft_wrapped_to_right_width() {
        let long_content = "x".repeat(200);
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine { kind: PatchLineKind::Added, text: long_content, old_line_no: None, new_line_no: Some(1) },
        ]);
        let context = ViewContext::new((120, 24));
        let right_width = 60;
        let (lines, refs) = build_patch_lines(&file, right_width, &context, &[]);

        // The long line should have wrapped into multiple visual lines
        assert!(lines.len() > 2, "long line should wrap, got {} lines", lines.len());

        // No visual line should exceed right_width
        for (i, line) in lines.iter().enumerate() {
            let w = line.display_width();
            assert!(w <= right_width, "line {i} width {w} exceeds right_width {right_width}: {}", line.plain_text());
        }

        // First wrapped line gets the ref, continuations get None
        assert!(refs[1].is_some(), "first wrapped line should have a ref");
        for (i, r) in refs.iter().enumerate().skip(2) {
            assert!(r.is_none(), "continuation line {i} should have None ref");
        }
    }

    #[test]
    fn short_lines_not_wrapped() {
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine {
                kind: PatchLineKind::Context,
                text: "short".to_string(),
                old_line_no: Some(1),
                new_line_no: Some(1),
            },
        ]);
        let context = ViewContext::new((120, 24));
        let (lines, refs) = build_patch_lines(&file, 80, &context, &[]);

        assert_eq!(lines.len(), 2, "short lines should not wrap");
        assert!(refs[0].is_some());
        assert!(refs[1].is_some());
    }

    #[test]
    fn wrapped_lines_extend_bg_to_width() {
        let long_content = "x".repeat(200);
        let file = make_file(vec![PatchLine {
            kind: PatchLineKind::Added,
            text: long_content,
            old_line_no: None,
            new_line_no: Some(1),
        }]);
        let context = ViewContext::new((120, 24));
        let right_width = 60;
        let (lines, _) = build_patch_lines(&file, right_width, &context, &[]);

        // All lines from the added line should have consistent width due to bg extension
        for line in &lines {
            let w = display_width_text(&line.plain_text());
            assert_eq!(w, right_width, "line should be padded to right_width: {}", line.plain_text());
        }
    }

    #[test]
    fn digit_count_works() {
        assert_eq!(digit_count(0), 1);
        assert_eq!(digit_count(1), 1);
        assert_eq!(digit_count(9), 1);
        assert_eq!(digit_count(10), 2);
        assert_eq!(digit_count(99), 2);
        assert_eq!(digit_count(100), 3);
        assert_eq!(digit_count(999), 3);
    }

    #[test]
    fn lang_hint_extracts_extension() {
        assert_eq!(lang_hint_from_path("src/main.rs"), "rs");
        assert_eq!(lang_hint_from_path("foo.py"), "py");
        assert_eq!(lang_hint_from_path("Makefile"), "Makefile");
        assert_eq!(lang_hint_from_path("a/b/c.tsx"), "tsx");
    }

    #[test]
    fn inline_comment_renders_below_target_line() {
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine {
                kind: PatchLineKind::Added,
                text: "new_code();".to_string(),
                old_line_no: None,
                new_line_no: Some(1),
            },
        ]);
        let comments = vec![QueuedComment {
            file_path: "test.rs".to_string(),
            patch_ref: PatchLineRef { hunk_index: 0, line_index: 1 },
            line_text: "new_code();".to_string(),
            line_number: Some(1),
            line_kind: PatchLineKind::Added,
            comment: "looks good".to_string(),
        }];
        let context = ViewContext::new((120, 24));
        let (lines, refs) = build_patch_lines(&file, 80, &context, &comments);

        // Expect: header, added line, comment top border, comment content, comment bottom border
        assert!(lines.len() > 2, "should have more lines with comment, got {}", lines.len());

        let added_row = 1;
        let comment_start = added_row + 1;
        let comment_text = lines[comment_start + 1].plain_text();
        assert!(
            comment_text.contains("looks good"),
            "comment content should contain 'looks good', got: {comment_text}"
        );

        let top_border = lines[comment_start].plain_text();
        assert!(top_border.contains('┌'), "comment top border should have ┌, got: {top_border}");

        let bottom_border = lines[comment_start + 2].plain_text();
        assert!(bottom_border.contains('└'), "comment bottom border should have └, got: {bottom_border}");

        // Comment rows should have None in refs
        for (i, r) in refs.iter().enumerate().take(comment_start + 3).skip(comment_start) {
            assert!(r.is_none(), "comment row {i} should have None ref");
        }
    }

    #[test]
    fn inline_comments_after_wrapped_rows() {
        let long_line = "x".repeat(200);
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine { kind: PatchLineKind::Added, text: long_line, old_line_no: None, new_line_no: Some(1) },
        ]);
        let comments = vec![QueuedComment {
            file_path: "test.rs".to_string(),
            patch_ref: PatchLineRef { hunk_index: 0, line_index: 1 },
            line_text: "long line".to_string(),
            line_number: Some(1),
            line_kind: PatchLineKind::Added,
            comment: "comment on wrapped".to_string(),
        }];
        let context = ViewContext::new((120, 24));
        let (lines, refs) = build_patch_lines(&file, 60, &context, &comments);

        // Find first comment row (should have ┌)
        let comment_top =
            lines.iter().position(|l| l.plain_text().contains('┌')).expect("should find comment top border");
        // All rows before comment_top (except header at 0) should be wrapped rows or the main line
        // The first wrapped row should have a ref, continuation rows should have None
        assert!(refs[1].is_some(), "first wrapped row should have ref");

        // Comment rows should all be None
        for (i, r) in refs.iter().enumerate().skip(comment_top) {
            assert!(r.is_none(), "comment row {i} should have None ref");
        }
    }

    #[test]
    fn multiple_comments_same_line_preserve_queue_order() {
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine {
                kind: PatchLineKind::Added,
                text: "code();".to_string(),
                old_line_no: None,
                new_line_no: Some(1),
            },
        ]);
        let comments = vec![
            QueuedComment {
                file_path: "test.rs".to_string(),
                patch_ref: PatchLineRef { hunk_index: 0, line_index: 1 },
                line_text: "code();".to_string(),
                line_number: Some(1),
                line_kind: PatchLineKind::Added,
                comment: "first".to_string(),
            },
            QueuedComment {
                file_path: "test.rs".to_string(),
                patch_ref: PatchLineRef { hunk_index: 0, line_index: 1 },
                line_text: "code();".to_string(),
                line_number: Some(1),
                line_kind: PatchLineKind::Added,
                comment: "second".to_string(),
            },
        ];
        let context = ViewContext::new((120, 24));
        let (lines, _refs) = build_patch_lines(&file, 80, &context, &comments);

        let text: Vec<String> = lines.iter().map(tui::Line::plain_text).collect();
        let first_pos = text.iter().position(|t| t.contains("first")).expect("should find 'first' comment");
        let second_pos = text.iter().position(|t| t.contains("second")).expect("should find 'second' comment");
        assert!(first_pos < second_pos, "first comment should appear before second");
    }

    #[test]
    fn long_comment_text_wraps() {
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine {
                kind: PatchLineKind::Added,
                text: "code();".to_string(),
                old_line_no: None,
                new_line_no: Some(1),
            },
        ]);
        let long_comment = "word ".repeat(50);
        let comments = vec![QueuedComment {
            file_path: "test.rs".to_string(),
            patch_ref: PatchLineRef { hunk_index: 0, line_index: 1 },
            line_text: "code();".to_string(),
            line_number: Some(1),
            line_kind: PatchLineKind::Added,
            comment: long_comment.trim().to_string(),
        }];
        let context = ViewContext::new((120, 24));
        let (lines, _refs) = build_patch_lines(&file, 40, &context, &comments);

        let content_rows: Vec<_> = lines.iter().skip(2).filter(|l| l.plain_text().contains("word")).collect();
        assert!(content_rows.len() > 1, "long comment should wrap into multiple rows, got {}", content_rows.len());
    }

    #[test]
    fn wrap_text_basic() {
        let result = wrap_text("hello world foo bar", 10);
        assert_eq!(result, vec!["hello", "world foo", "bar"]);
    }

    #[test]
    fn wrap_text_empty() {
        let result = wrap_text("", 10);
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn wrap_text_single_word_fits() {
        let result = wrap_text("hello", 10);
        assert_eq!(result, vec!["hello"]);
    }

    #[test]
    fn wrap_text_zero_width() {
        let result = wrap_text("hello", 0);
        assert_eq!(result, vec![""]);
    }
}
