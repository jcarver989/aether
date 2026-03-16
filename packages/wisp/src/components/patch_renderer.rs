use crate::components::app::git_diff_mode::PatchLineRef;
use crate::git_diff::{FileDiff, PatchLineKind};
use tui::{Color, Line, Span, Style, ViewContext};

pub fn build_patch_lines(
    file: &FileDiff,
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
                    line.push_with_style(&pl.text, Style::fg(theme.info()).bold());
                }
                PatchLineKind::Context => {
                    let old_str = format_line_no(pl.old_line_no, gutter_width);
                    let new_str = format_line_no(pl.new_line_no, gutter_width);
                    line.push_with_style(
                        format!("{old_str} {new_str}   "),
                        Style::fg(theme.text_secondary()),
                    );
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
                    let style =
                        Style::fg(theme.diff_removed_fg()).bg_color(theme.diff_removed_bg());
                    line.push_with_style(format!("{old_str} {new_str} - "), style);
                    append_syntax_spans(&mut line, &pl.text, lang_hint, bg, context);
                }
                PatchLineKind::Meta => {
                    line.push_with_style(&pl.text, Style::fg(theme.text_secondary()).italic());
                }
            }

            patch_lines.push(line);
            patch_refs.push(Some(PatchLineRef {
                hunk_index: hunk_idx,
                line_index: line_idx,
            }));
        }
    }

    (patch_lines, patch_refs)
}

pub(crate) fn lang_hint_from_path(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or("")
}

fn append_syntax_spans(
    line: &mut Line,
    text: &str,
    lang_hint: &str,
    bg_override: Option<Color>,
    context: &ViewContext,
) {
    let spans = context
        .highlighter()
        .highlight(text, lang_hint, &context.theme);
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

fn format_line_no(line_no: Option<usize>, width: usize) -> String {
    match line_no {
        Some(n) => format!("{n:>width$}"),
        None => " ".repeat(width),
    }
}

fn digit_count(mut n: usize) -> usize {
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
