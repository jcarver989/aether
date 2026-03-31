use crate::components::app::git_diff_mode::PatchLineRef;
use crate::git_diff::{FileDiff, PatchLineKind};
use tui::{Color, Line, Span, Style, ViewContext, soft_wrap_line};

pub fn build_patch_lines(
    file: &FileDiff,
    right_width: usize,
    context: &ViewContext,
) -> (Vec<Line>, Vec<Option<PatchLineRef>>) {
    let theme = &context.theme;
    let lang_hint = lang_hint_from_path(&file.path);
    let mut patch_lines = Vec::new();
    let mut patch_refs = Vec::new();

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

            #[allow(clippy::cast_possible_truncation)]
            let wrapped = soft_wrap_line(&line, right_width as u16);
            for (i, mut wrapped_line) in wrapped.into_iter().enumerate() {
                wrapped_line.extend_bg_to_width(right_width);
                patch_lines.push(wrapped_line);
                if i == 0 {
                    patch_refs.push(Some(PatchLineRef { hunk_index: hunk_idx, line_index: line_idx }));
                } else {
                    patch_refs.push(None);
                }
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
        let (lines, refs) = build_patch_lines(&file, right_width, &context);

        // The long line should have wrapped into multiple visual lines
        assert!(lines.len() > 2, "long line should wrap, got {} lines", lines.len());

        // No visual line should exceed right_width
        for (i, line) in lines.iter().enumerate() {
            let w = line.display_width();
            assert!(w <= right_width, "line {i} width {w} exceeds right_width {right_width}: {}", line.plain_text());
        }

        // First wrapped line gets the ref, continuations get None
        assert!(refs[1].is_some(), "first wrapped line should have a ref");
        for i in 2..lines.len() {
            assert!(refs[i].is_none(), "continuation line {i} should have None ref");
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
        let (lines, refs) = build_patch_lines(&file, 80, &context);

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
        let (lines, _) = build_patch_lines(&file, right_width, &context);

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
}
