use crate::components::app::git_diff_mode::PatchLineRef;
use crate::components::patch_renderer::lang_hint_from_path;
use crate::git_diff::{FileDiff, PatchLine, PatchLineKind};
use similar::{ChangeTag, DiffOp as SimilarDiffOp, TextDiff};
use tui::{
    DiffTag, GUTTER_WIDTH, Line, SEPARATOR, SEPARATOR_WIDTH, SplitDiffCell, Style, ViewContext,
    split_blank_panel, split_render_cell,
};

pub fn build_split_patch_lines(
    file: &FileDiff,
    right_width: usize,
    context: &ViewContext,
) -> (Vec<Line>, Vec<Option<PatchLineRef>>) {
    let theme = &context.theme;
    let lang_hint = lang_hint_from_path(&file.path);

    let usable = right_width.saturating_sub(GUTTER_WIDTH * 2 + SEPARATOR_WIDTH);
    let left_content = usable / 2;
    let right_content = usable.saturating_sub(left_content);
    let left_panel = GUTTER_WIDTH + left_content;
    let right_panel = GUTTER_WIDTH + right_content;

    let mut patch_lines = Vec::new();
    let mut patch_refs = Vec::new();

    for (hunk_idx, hunk) in file.hunks.iter().enumerate() {
        if hunk_idx > 0 {
            patch_lines.push(Line::default());
            patch_refs.push(None);
        }

        let rows = pair_hunk_lines(&hunk.lines);

        for row in &rows {
            match row {
                PairedRow::Header { line_idx, text } => {
                    let mut line = Line::default();
                    line.push_with_style(
                        *text,
                        Style::fg(theme.info()).bold().bg_color(theme.code_bg()),
                    );
                    line.extend_bg_to_width(right_width);
                    patch_lines.push(line);
                    patch_refs.push(Some(PatchLineRef {
                        hunk_index: hunk_idx,
                        line_index: *line_idx,
                    }));
                }
                PairedRow::Split {
                    left,
                    right,
                    left_highlights,
                    right_highlights,
                } => {
                    let left_cell = left.as_ref().map(|s| SplitDiffCell {
                        tag: match s.kind {
                            PatchLineKind::Removed => DiffTag::Removed,
                            _ => DiffTag::Context,
                        },
                        content: s.text.to_string(),
                        line_number: s.line_no,
                        highlights: left_highlights.clone(),
                    });
                    let right_cell = right.as_ref().map(|s| SplitDiffCell {
                        tag: match s.kind {
                            PatchLineKind::Added => DiffTag::Added,
                            _ => DiffTag::Context,
                        },
                        content: s.text.to_string(),
                        line_number: s.line_no,
                        highlights: right_highlights.clone(),
                    });

                    let left_rendered =
                        split_render_cell(left_cell.as_ref(), left_content, lang_hint, context);
                    let right_rendered =
                        split_render_cell(right_cell.as_ref(), right_content, lang_hint, context);

                    let height = left_rendered.len().max(right_rendered.len());

                    let patch_ref = right
                        .as_ref()
                        .map(|s| PatchLineRef {
                            hunk_index: hunk_idx,
                            line_index: s.line_idx,
                        })
                        .or_else(|| {
                            left.as_ref().map(|s| PatchLineRef {
                                hunk_index: hunk_idx,
                                line_index: s.line_idx,
                            })
                        });

                    for i in 0..height {
                        let l = left_rendered
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| split_blank_panel(left_panel));
                        let r = right_rendered
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| split_blank_panel(right_panel));

                        let mut line = l;
                        line.push_styled(SEPARATOR, theme.muted());
                        line.append_line(&r);
                        patch_lines.push(line);

                        if i == 0 {
                            patch_refs.push(patch_ref.clone());
                        } else {
                            patch_refs.push(None);
                        }
                    }
                }
            }
        }
    }

    (patch_lines, patch_refs)
}

#[derive(Clone, Copy)]
struct SideInfo<'a> {
    kind: PatchLineKind,
    text: &'a str,
    line_no: Option<usize>,
    line_idx: usize,
}

enum PairedRow<'a> {
    Header {
        line_idx: usize,
        text: &'a str,
    },
    Split {
        left: Option<SideInfo<'a>>,
        right: Option<SideInfo<'a>>,
        left_highlights: HighlightRanges,
        right_highlights: HighlightRanges,
    },
}

fn pair_hunk_lines(lines: &[PatchLine]) -> Vec<PairedRow<'_>> {
    let mut rows = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let pl = &lines[i];
        match pl.kind {
            PatchLineKind::HunkHeader | PatchLineKind::Meta => {
                rows.push(PairedRow::Header {
                    line_idx: i,
                    text: &pl.text,
                });
                i += 1;
            }
            PatchLineKind::Context => {
                rows.push(PairedRow::Split {
                    left: Some(SideInfo {
                        kind: pl.kind,
                        text: &pl.text,
                        line_no: pl.old_line_no,
                        line_idx: i,
                    }),
                    right: Some(SideInfo {
                        kind: pl.kind,
                        text: &pl.text,
                        line_no: pl.new_line_no,
                        line_idx: i,
                    }),
                    left_highlights: vec![],
                    right_highlights: vec![],
                });
                i += 1;
            }
            PatchLineKind::Removed => {
                let mut removed = Vec::new();
                while i < lines.len() && lines[i].kind == PatchLineKind::Removed {
                    removed.push(side_info(&lines[i], i));
                    i += 1;
                }
                let mut added = Vec::new();
                while i < lines.len() && lines[i].kind == PatchLineKind::Added {
                    added.push(side_info(&lines[i], i));
                    i += 1;
                }

                rows.extend(pair_changed_block(&removed, &added));
            }
            PatchLineKind::Added => {
                let mut added = Vec::new();
                while i < lines.len() && lines[i].kind == PatchLineKind::Added {
                    added.push(side_info(&lines[i], i));
                    i += 1;
                }
                rows.extend(pair_changed_block(&[], &added));
            }
        }
    }

    rows
}

type HighlightRanges = Vec<(usize, usize)>;

fn side_info(line: &PatchLine, line_idx: usize) -> SideInfo<'_> {
    let line_no = match line.kind {
        PatchLineKind::Added => line.new_line_no,
        PatchLineKind::Removed | PatchLineKind::Context => line.old_line_no,
        PatchLineKind::HunkHeader | PatchLineKind::Meta => None,
    };

    SideInfo {
        kind: line.kind,
        text: &line.text,
        line_no,
        line_idx,
    }
}

fn pair_changed_block<'a>(removed: &[SideInfo<'a>], added: &[SideInfo<'a>]) -> Vec<PairedRow<'a>> {
    let old: Vec<&str> = removed.iter().map(|info| info.text).collect();
    let new: Vec<&str> = added.iter().map(|info| info.text).collect();
    let diff = TextDiff::from_slices(&old, &new);
    let mut rows = Vec::new();

    for op in diff.ops() {
        match *op {
            SimilarDiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                for offset in 0..len {
                    rows.push(split_row(
                        Some(removed[old_index + offset]),
                        Some(added[new_index + offset]),
                        vec![],
                        vec![],
                    ));
                }
            }
            SimilarDiffOp::Delete {
                old_index, old_len, ..
            } => {
                for side in &removed[old_index..old_index + old_len] {
                    rows.push(split_row(Some(*side), None, vec![], vec![]));
                }
            }
            SimilarDiffOp::Insert {
                new_index, new_len, ..
            } => {
                for side in &added[new_index..new_index + new_len] {
                    rows.push(split_row(None, Some(*side), vec![], vec![]));
                }
            }
            SimilarDiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let pair_len = old_len.min(new_len);
                let allow_inline_highlights = old_len == new_len;

                for offset in 0..pair_len {
                    let left = removed[old_index + offset];
                    let right = added[new_index + offset];
                    let (left_highlights, right_highlights) = if allow_inline_highlights {
                        compute_word_highlights(Some(&left), Some(&right))
                    } else {
                        (vec![], vec![])
                    };
                    rows.push(split_row(
                        Some(left),
                        Some(right),
                        left_highlights,
                        right_highlights,
                    ));
                }

                for side in &removed[old_index + pair_len..old_index + old_len] {
                    rows.push(split_row(Some(*side), None, vec![], vec![]));
                }
                for side in &added[new_index + pair_len..new_index + new_len] {
                    rows.push(split_row(None, Some(*side), vec![], vec![]));
                }
            }
        }
    }

    rows
}

fn split_row<'a>(
    left: Option<SideInfo<'a>>,
    right: Option<SideInfo<'a>>,
    left_highlights: HighlightRanges,
    right_highlights: HighlightRanges,
) -> PairedRow<'a> {
    PairedRow::Split {
        left,
        right,
        left_highlights,
        right_highlights,
    }
}

fn compute_word_highlights(
    left: Option<&SideInfo>,
    right: Option<&SideInfo>,
) -> (HighlightRanges, HighlightRanges) {
    let (Some(l), Some(r)) = (left, right) else {
        return (vec![], vec![]);
    };
    if l.kind != PatchLineKind::Removed || r.kind != PatchLineKind::Added {
        return (vec![], vec![]);
    }

    let diff = TextDiff::from_chars(l.text, r.text);
    let mut left_hl = Vec::new();
    let mut right_hl = Vec::new();
    let mut old_pos = 0usize;
    let mut new_pos = 0usize;

    for change in diff.iter_all_changes() {
        let len = change.value().len();
        match change.tag() {
            ChangeTag::Equal => {
                old_pos += len;
                new_pos += len;
            }
            ChangeTag::Delete => {
                left_hl.push((old_pos, old_pos + len));
                old_pos += len;
            }
            ChangeTag::Insert => {
                right_hl.push((new_pos, new_pos + len));
                new_pos += len;
            }
        }
    }

    (merge_ranges(left_hl), merge_ranges(right_hl))
}

fn merge_ranges(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if ranges.len() <= 1 {
        return ranges;
    }
    ranges.sort_by_key(|r| r.0);
    let mut merged = vec![ranges[0]];
    for &(start, end) in &ranges[1..] {
        let last = merged.last_mut().unwrap();
        if start <= last.1 {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_diff::{FileDiff, FileStatus, Hunk, PatchLine, PatchLineKind};

    fn pl(kind: PatchLineKind, text: &str, old: Option<usize>, new: Option<usize>) -> PatchLine {
        PatchLine {
            kind,
            text: text.to_string(),
            old_line_no: old,
            new_line_no: new,
        }
    }

    fn test_file(hunks: Vec<Hunk>) -> FileDiff {
        FileDiff {
            old_path: Some("test.rs".to_string()),
            path: "test.rs".to_string(),
            status: FileStatus::Modified,
            hunks,
            binary: false,
        }
    }

    fn test_hunk(lines: Vec<PatchLine>) -> Hunk {
        Hunk {
            header: "@@ -1,3 +1,3 @@".to_string(),
            old_start: 1,
            old_count: 3,
            new_start: 1,
            new_count: 3,
            lines,
        }
    }

    fn ctx() -> ViewContext {
        ViewContext::new((120, 24))
    }

    #[test]
    fn split_pairs_removed_and_added() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,3 +1,3 @@", None, None),
            pl(PatchLineKind::Removed, "old_a", Some(1), None),
            pl(PatchLineKind::Removed, "old_b", Some(2), None),
            pl(PatchLineKind::Added, "new_a", None, Some(1)),
            pl(PatchLineKind::Added, "new_b", None, Some(2)),
        ])]);
        let (lines, refs) = build_split_patch_lines(&file, 100, &ctx());
        // 1 header + 2 paired rows
        assert_eq!(lines.len(), 3);
        assert_eq!(refs.len(), 3);
        // Both paired rows should have refs
        assert!(refs[1].is_some());
        assert!(refs[2].is_some());
    }

    #[test]
    fn split_uneven_removed_and_added() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,3 +1,1 @@", None, None),
            pl(PatchLineKind::Removed, "old_a", Some(1), None),
            pl(PatchLineKind::Removed, "old_b", Some(2), None),
            pl(PatchLineKind::Removed, "old_c", Some(3), None),
            pl(PatchLineKind::Added, "new_a", None, Some(1)),
        ])]);
        let (lines, refs) = build_split_patch_lines(&file, 100, &ctx());
        // 1 header + 3 rows (first paired, last 2 have only left)
        assert_eq!(lines.len(), 4);
        // Verify the unpaired rows still have refs (pointing to left side)
        assert!(refs[2].is_some());
        assert!(refs[3].is_some());
    }

    #[test]
    fn split_context_on_both_sides() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,1 +1,1 @@", None, None),
            pl(PatchLineKind::Context, "same line", Some(1), Some(1)),
        ])]);
        let (lines, _refs) = build_split_patch_lines(&file, 100, &ctx());
        // 1 header + 1 context
        assert_eq!(lines.len(), 2);
        let text = lines[1].plain_text();
        // Context text should appear twice (both panels)
        let count = text.matches("same line").count();
        assert_eq!(count, 2, "context should appear on both sides: {text}");
    }

    #[test]
    fn split_hunk_header_full_width() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,1 +1,1 @@", None, None),
            pl(PatchLineKind::Context, "x", Some(1), Some(1)),
        ])]);
        let (lines, refs) = build_split_patch_lines(&file, 100, &ctx());
        let header_text = lines[0].plain_text();
        assert!(
            header_text.contains("@@ -1,1 +1,1 @@"),
            "header missing: {header_text}"
        );
        assert!(refs[0].is_some());
    }

    #[test]
    fn split_spacer_between_hunks() {
        let file = test_file(vec![
            test_hunk(vec![
                pl(PatchLineKind::HunkHeader, "@@ -1,1 +1,1 @@", None, None),
                pl(PatchLineKind::Context, "a", Some(1), Some(1)),
            ]),
            Hunk {
                header: "@@ -5,1 +5,1 @@".to_string(),
                old_start: 5,
                old_count: 1,
                new_start: 5,
                new_count: 1,
                lines: vec![
                    pl(PatchLineKind::HunkHeader, "@@ -5,1 +5,1 @@", None, None),
                    pl(PatchLineKind::Context, "b", Some(5), Some(5)),
                ],
            },
        ]);
        let (_lines, refs) = build_split_patch_lines(&file, 100, &ctx());
        // Layout: hunk1_header, context_a, spacer, hunk2_header, context_b
        assert_eq!(refs.len(), 5);
        assert!(
            refs[2].is_none(),
            "spacer between hunks should have None ref"
        );
    }

    #[test]
    fn split_refs_prefer_right_side() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,1 +1,1 @@", None, None),
            pl(PatchLineKind::Removed, "old", Some(1), None),
            pl(PatchLineKind::Added, "new", None, Some(1)),
        ])]);
        let (_lines, refs) = build_split_patch_lines(&file, 100, &ctx());
        // The paired row ref should point to the Added line (index 2 in the hunk)
        let paired_ref = refs[1].as_ref().unwrap();
        assert_eq!(paired_ref.line_index, 2, "should reference the Added line");
    }

    #[test]
    fn word_highlights_computed_for_paired_lines() {
        let left = SideInfo {
            kind: PatchLineKind::Removed,
            text: "let x = foo();",
            line_no: Some(1),
            line_idx: 0,
        };
        let right = SideInfo {
            kind: PatchLineKind::Added,
            text: "let x = bar();",
            line_no: Some(1),
            line_idx: 1,
        };
        let (left_hl, right_hl) = compute_word_highlights(Some(&left), Some(&right));
        // "foo" (bytes 8..11) differs from "bar" (bytes 8..11)
        assert_eq!(left_hl, vec![(8, 11)]);
        assert_eq!(right_hl, vec![(8, 11)]);
    }

    #[test]
    fn no_highlights_for_context_lines() {
        let left = SideInfo {
            kind: PatchLineKind::Context,
            text: "same line",
            line_no: Some(1),
            line_idx: 0,
        };
        let right = SideInfo {
            kind: PatchLineKind::Context,
            text: "same line",
            line_no: Some(1),
            line_idx: 0,
        };
        let (left_hl, right_hl) = compute_word_highlights(Some(&left), Some(&right));
        assert!(left_hl.is_empty());
        assert!(right_hl.is_empty());
    }

    #[test]
    fn no_highlights_when_unpaired() {
        let left = SideInfo {
            kind: PatchLineKind::Removed,
            text: "deleted",
            line_no: Some(1),
            line_idx: 0,
        };
        let (left_hl, right_hl) = compute_word_highlights(Some(&left), None);
        assert!(left_hl.is_empty());
        assert!(right_hl.is_empty());
    }

    #[test]
    fn ambiguous_multi_line_block_skips_word_highlights() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,2 +1,2 @@", None, None),
            pl(
                PatchLineKind::Removed,
                "patch_lines.push(line);",
                Some(1),
                None,
            ),
            pl(
                PatchLineKind::Removed,
                "patch_refs.push(Some(PatchLineRef {",
                Some(2),
                None,
            ),
            pl(
                PatchLineKind::Added,
                "patch_refs.push(Some(PatchLineRef {",
                None,
                Some(1),
            ),
            pl(
                PatchLineKind::Added,
                "patch_lines.push(line);",
                None,
                Some(2),
            ),
        ])]);
        let context = ctx();
        let (lines, _refs) = build_split_patch_lines(&file, 100, &context);
        let removed_highlight = context.theme.diff_removed_highlight_bg();
        let added_highlight = context.theme.diff_added_highlight_bg();

        let has_word_highlight = lines.iter().skip(1).any(|line| {
            line.spans().iter().any(|span| {
                matches!(
                    span.style().bg,
                    Some(bg) if bg == removed_highlight || bg == added_highlight
                )
            })
        });

        assert!(
            !has_word_highlight,
            "ambiguous multi-line blocks should not render word highlights"
        );
    }

    #[test]
    fn straightforward_multi_line_block_keeps_word_highlights() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,2 +1,2 @@", None, None),
            pl(PatchLineKind::Removed, "let alpha = foo();", Some(1), None),
            pl(PatchLineKind::Removed, "let beta = baz();", Some(2), None),
            pl(PatchLineKind::Added, "let alpha = bar();", None, Some(1)),
            pl(PatchLineKind::Added, "let beta = qux();", None, Some(2)),
        ])]);
        let context = ctx();
        let (lines, _refs) = build_split_patch_lines(&file, 100, &context);
        let removed_highlight = context.theme.diff_removed_highlight_bg();
        let added_highlight = context.theme.diff_added_highlight_bg();

        let highlight_rows = lines
            .iter()
            .skip(1)
            .filter(|line| {
                line.spans().iter().any(|span| {
                    matches!(
                        span.style().bg,
                        Some(bg) if bg == removed_highlight || bg == added_highlight
                    )
                })
            })
            .count();

        assert_eq!(
            highlight_rows, 2,
            "each paired row should retain word highlights for straightforward replacements"
        );
    }

    #[test]
    fn reflowed_multi_line_block_skips_word_highlights() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,3 +1,2 @@", None, None),
            pl(
                PatchLineKind::Removed,
                ".send_cancellable_request(",
                Some(1),
                None,
            ),
            pl(
                PatchLineKind::Removed,
                "CallToolRequest(Request::new(params)),",
                Some(2),
                None,
            ),
            pl(PatchLineKind::Removed, "{", Some(3), None),
            pl(
                PatchLineKind::Added,
                ".send_cancellable_request(CallToolRequest(Request::new(params)), {",
                None,
                Some(1),
            ),
            pl(
                PatchLineKind::Added,
                "let mut opts = PeerRequestOptions::default();",
                None,
                Some(2),
            ),
        ])]);
        let context = ctx();
        let (lines, _refs) = build_split_patch_lines(&file, 100, &context);
        let removed_highlight = context.theme.diff_removed_highlight_bg();
        let added_highlight = context.theme.diff_added_highlight_bg();

        let has_word_highlight = lines.iter().skip(1).any(|line| {
            line.spans().iter().any(|span| {
                matches!(
                    span.style().bg,
                    Some(bg) if bg == removed_highlight || bg == added_highlight
                )
            })
        });

        assert!(
            !has_word_highlight,
            "reflowed multi-line blocks should not render word highlights"
        );
    }

    #[test]
    fn moved_identical_line_is_aligned_on_same_row() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,2 +1,2 @@", None, None),
            pl(PatchLineKind::Removed, "let before = old();", Some(1), None),
            pl(PatchLineKind::Removed, "shared_call();", Some(2), None),
            pl(PatchLineKind::Added, "shared_call();", None, Some(1)),
            pl(PatchLineKind::Added, "let after = new();", None, Some(2)),
        ])]);
        let (lines, _refs) = build_split_patch_lines(&file, 100, &ctx());

        let shared_row = lines
            .iter()
            .skip(1)
            .find(|line| line.plain_text().matches("shared_call();").count() == 2);

        assert!(
            shared_row.is_some(),
            "identical moved lines should be aligned onto the same split row"
        );
    }

    #[test]
    fn replace_with_extra_added_line_keeps_prefix_aligned_and_overflow_unpaired() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,2 +1,3 @@", None, None),
            pl(PatchLineKind::Removed, "shared_call();", Some(1), None),
            pl(
                PatchLineKind::Removed,
                "let old_value = foo();",
                Some(2),
                None,
            ),
            pl(PatchLineKind::Added, "shared_call();", None, Some(1)),
            pl(
                PatchLineKind::Added,
                "let new_value = bar();",
                None,
                Some(2),
            ),
            pl(PatchLineKind::Added, "extra_call();", None, Some(3)),
        ])]);
        let (lines, _refs) = build_split_patch_lines(&file, 100, &ctx());

        let shared_row = lines
            .iter()
            .skip(1)
            .find(|line| line.plain_text().matches("shared_call();").count() == 2);
        assert!(
            shared_row.is_some(),
            "shared prefix line should stay aligned as an unchanged pair"
        );

        let overflow_row = lines
            .iter()
            .skip(1)
            .find(|line| line.plain_text().contains("extra_call();"))
            .expect("expected overflow added row");
        assert_eq!(
            overflow_row.plain_text().matches("extra_call();").count(),
            1,
            "overflow added line should remain unpaired"
        );
    }
}
