use crate::components::app::git_diff_mode::PatchLineRef;
use crate::git_diff::{FileDiff, PatchLineKind};
use std::collections::HashMap;
use tui::{Line, ViewContext};

pub struct RenderedPatch {
    pub lines: Vec<Line>,
    pub line_refs: Vec<Option<PatchLineRef>>,
    pub line_ref_to_anchor_row_index: HashMap<PatchLineRef, usize>,
    pub hunk_offsets: Vec<usize>,
}

impl RenderedPatch {
    pub(crate) fn new(
        lines: Vec<Line>,
        line_refs: Vec<Option<PatchLineRef>>,
        line_ref_to_anchor_row_index: HashMap<PatchLineRef, usize>,
    ) -> Self {
        let hunk_offsets = compute_hunk_offsets(&line_refs);
        Self { lines, line_refs, line_ref_to_anchor_row_index, hunk_offsets }
    }

    pub fn from_file_diff(file: &FileDiff, width: usize, ctx: &ViewContext) -> RenderedPatch {
        let theme = &ctx.theme;
        let lang_hint = lang_hint_from_path(&file.path);
        let mut patch_lines = Vec::new();
        let mut patch_refs = Vec::new();
        let mut anchor_insert_row_lookup = HashMap::new();
        let max_line_no = file
            .hunks
            .iter()
            .flat_map(|hunk| &hunk.lines)
            .filter_map(|line| line.old_line_no.into_iter().chain(line.new_line_no).max())
            .max()
            .unwrap_or(0);

        let gutter_width = get_n_digits(max_line_no);

        for (hunk_idx, hunk) in file.hunks.iter().enumerate() {
            if hunk_idx > 0 {
                patch_lines.push(Line::default());
                patch_refs.push(None);
            }

            for (line_idx, patch_line) in hunk.lines.iter().enumerate() {
                let mut line = Line::default();

                match patch_line.kind {
                    PatchLineKind::HunkHeader => {
                        line.push_with_style(
                            &patch_line.text,
                            tui::Style::fg(theme.info()).bold().bg_color(theme.code_bg()),
                        );
                    }
                    PatchLineKind::Context => {
                        let old_str = format_line_no(patch_line.old_line_no, gutter_width);
                        let new_str = format_line_no(patch_line.new_line_no, gutter_width);
                        line.push_with_style(format!("{old_str} {new_str}   "), tui::Style::fg(theme.text_secondary()));
                        append_syntax_spans(&mut line, &patch_line.text, lang_hint, None, ctx);
                    }

                    PatchLineKind::Added => {
                        let old_str = " ".repeat(gutter_width);
                        let new_str = format_line_no(patch_line.new_line_no, gutter_width);
                        let bg = Some(theme.diff_added_bg());
                        let style = tui::Style::fg(theme.diff_added_fg()).bg_color(theme.diff_added_bg());
                        line.push_with_style(format!("{old_str} {new_str} + "), style);
                        append_syntax_spans(&mut line, &patch_line.text, lang_hint, bg, ctx);
                    }
                    PatchLineKind::Removed => {
                        let old_str = format_line_no(patch_line.old_line_no, gutter_width);
                        let new_str = " ".repeat(gutter_width);
                        let bg = Some(theme.diff_removed_bg());
                        let style = tui::Style::fg(theme.diff_removed_fg()).bg_color(theme.diff_removed_bg());
                        line.push_with_style(format!("{old_str} {new_str} - "), style);
                        append_syntax_spans(&mut line, &patch_line.text, lang_hint, bg, ctx);
                    }
                    PatchLineKind::Meta => {
                        line.push_with_style(&patch_line.text, tui::Style::fg(theme.text_secondary()).italic());
                    }
                }

                let anchor = PatchLineRef { hunk_index: hunk_idx, line_index: line_idx };
                let width_u16 = usize_to_u16_saturating(width);
                let wrapped = tui::Frame::new(vec![line])
                    .fit(width_u16, tui::FitOptions::wrap())
                    .map_lines(|mut wrapped_line| {
                        wrapped_line.extend_bg_to_width(width);
                        wrapped_line
                    })
                    .into_lines();

                anchor_insert_row_lookup.insert(anchor, patch_lines.len() + wrapped.len());
                patch_refs.push(Some(anchor));
                patch_refs.extend(std::iter::repeat_n(None, wrapped.len().saturating_sub(1)));
                patch_lines.extend(wrapped);
            }
        }

        Self::new(patch_lines, patch_refs, anchor_insert_row_lookup)
    }
}

fn append_syntax_spans(
    line: &mut Line,
    text: &str,
    lang_hint: &str,
    bg_override: Option<tui::Color>,
    ctx: &ViewContext,
) {
    let spans = ctx.highlighter().highlight(text, lang_hint, &ctx.theme);
    if let Some(content) = spans.first() {
        for span in content.spans() {
            let mut span_style = span.style();
            if let Some(bg) = bg_override {
                span_style.bg = Some(bg);
            }
            line.push_span(tui::Span::with_style(span.text(), span_style));
        }
    } else {
        line.push_text(text);
    }
}

fn format_line_no(line_no: Option<usize>, width: usize) -> String {
    match line_no {
        Some(line_no) => format!("{line_no:>width$}"),
        None => " ".repeat(width),
    }
}

pub(crate) fn lang_hint_from_path(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or("")
}

pub(crate) fn usize_to_u16_saturating(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

fn compute_hunk_offsets(line_refs: &[Option<PatchLineRef>]) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut last_hunk: Option<usize> = None;

    for (index, line_ref) in line_refs.iter().enumerate() {
        if let Some(patch_line_ref) = line_ref
            && last_hunk != Some(patch_line_ref.hunk_index)
        {
            offsets.push(index);
            last_hunk = Some(patch_line_ref.hunk_index);
        }
    }

    offsets
}

fn get_n_digits(mut n: usize) -> usize {
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
    fn rendered_patch_contains_insert_row_lookup() {
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine {
                kind: PatchLineKind::Context,
                text: "fn test()".to_string(),
                old_line_no: Some(1),
                new_line_no: Some(1),
            },
        ]);
        let context = ViewContext::new((120, 24));
        let result = RenderedPatch::from_file_diff(&file, 80, &context);

        assert_eq!(result.line_ref_to_anchor_row_index.len(), 2);
        assert_eq!(result.line_ref_to_anchor_row_index[&PatchLineRef { hunk_index: 0, line_index: 0 }], 1);
    }

    #[test]
    fn rendered_patch_contains_hunk_offsets() {
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine {
                kind: PatchLineKind::Context,
                text: "fn test()".to_string(),
                old_line_no: Some(1),
                new_line_no: Some(1),
            },
        ]);
        let context = ViewContext::new((120, 24));
        let result = RenderedPatch::from_file_diff(&file, 80, &context);

        assert!(!result.hunk_offsets.is_empty(), "should have at least one hunk offset");
        assert_eq!(result.hunk_offsets[0], 0, "first hunk should start at row 0");
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
        let result = RenderedPatch::from_file_diff(&file, right_width, &context);

        assert!(result.lines.len() > 2, "long line should wrap, got {} lines", result.lines.len());

        for (index, line) in result.lines.iter().enumerate() {
            let display_width = line.display_width();
            assert!(
                display_width <= right_width,
                "line {index} width {display_width} exceeds right_width {right_width}"
            );
        }

        assert!(result.line_refs[1].is_some(), "first wrapped line should have a ref");
        for (index, line_ref) in result.line_refs.iter().enumerate().skip(2) {
            assert!(line_ref.is_none(), "continuation line {index} should have None ref");
        }
    }

    #[test]
    fn digit_count_works() {
        assert_eq!(get_n_digits(0), 1);
        assert_eq!(get_n_digits(1), 1);
        assert_eq!(get_n_digits(9), 1);
        assert_eq!(get_n_digits(10), 2);
        assert_eq!(get_n_digits(99), 2);
        assert_eq!(get_n_digits(100), 3);
        assert_eq!(get_n_digits(999), 3);
    }

    #[test]
    fn lang_hint_extracts_extension() {
        assert_eq!(lang_hint_from_path("src/main.rs"), "rs");
        assert_eq!(lang_hint_from_path("foo.py"), "py");
        assert_eq!(lang_hint_from_path("Makefile"), "Makefile");
        assert_eq!(lang_hint_from_path("a/b/c.tsx"), "tsx");
    }

    #[test]
    fn usize_to_u16_saturating_clamps_large_values() {
        assert_eq!(usize_to_u16_saturating(123), 123);
        assert_eq!(usize_to_u16_saturating(usize::from(u16::MAX)), u16::MAX);
        assert_eq!(usize_to_u16_saturating(usize::from(u16::MAX) + 1), u16::MAX);
    }
}
