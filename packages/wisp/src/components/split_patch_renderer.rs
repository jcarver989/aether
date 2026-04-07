use crate::components::app::git_diff_mode::{PatchLineRef, QueuedComment};
use crate::components::patch_renderer::{append_inline_comment_rows, build_comment_map, lang_hint_from_path};
use crate::git_diff::{FileDiff, PatchLine, PatchLineKind};
use similar::{DiffOp as SimilarDiffOp, TextDiff};
use tui::{
    DiffTag, Frame, FramePart, GUTTER_WIDTH, Line, SEPARATOR, SEPARATOR_WIDTH, SplitDiffCell, Style, ViewContext,
    split_render_cell,
};

const SEPARATOR_WIDTH_U16: u16 = 3;

pub fn build_split_patch_lines(
    file: &FileDiff,
    right_width: usize,
    context: &ViewContext,
    comments: &[QueuedComment],
) -> (Vec<Line>, Vec<Option<PatchLineRef>>) {
    let theme = &context.theme;
    let lang_hint = lang_hint_from_path(&file.path);

    let comment_map = build_comment_map(comments);

    let usable = right_width.saturating_sub(GUTTER_WIDTH * 2 + SEPARATOR_WIDTH);
    let left_content = usable / 2;
    let right_content = usable.saturating_sub(left_content);
    #[allow(clippy::cast_possible_truncation)]
    let left_panel_u16 = (GUTTER_WIDTH + left_content) as u16;
    #[allow(clippy::cast_possible_truncation)]
    let right_panel_u16 = (GUTTER_WIDTH + right_content) as u16;
    let sep_style = Style::fg(theme.muted()).bg_color(theme.background());

    let mut patch_lines = Vec::new();
    let mut patch_refs = Vec::new();

    for (hunk_idx, hunk) in file.hunks.iter().enumerate() {
        if hunk_idx > 0 {
            patch_lines.push(Line::default());
            patch_refs.push(None);
        }

        let rows = pair_hunk_lines(&hunk.lines);

        for row in &rows {
            let anchor = match row {
                PairedRow::Header { line_idx, .. } => {
                    Some(PatchLineRef { hunk_index: hunk_idx, line_index: *line_idx })
                }
                PairedRow::Split { right, left } => right
                    .as_ref()
                    .map(|s| PatchLineRef { hunk_index: hunk_idx, line_index: s.line_idx })
                    .or_else(|| left.as_ref().map(|s| PatchLineRef { hunk_index: hunk_idx, line_index: s.line_idx })),
            };

            match row {
                PairedRow::Header { text, .. } => {
                    let mut line = Line::default();
                    line.push_with_style(*text, Style::fg(theme.info()).bold().bg_color(theme.code_bg()));
                    line.extend_bg_to_width(right_width);
                    patch_lines.push(line);
                    patch_refs.push(anchor);
                }
                PairedRow::Split { left, right } => {
                    let left_cell = left.as_ref().map(|s| SplitDiffCell {
                        tag: match s.kind {
                            PatchLineKind::Removed => DiffTag::Removed,
                            _ => DiffTag::Context,
                        },
                        content: s.text.to_string(),
                        line_number: s.line_no,
                    });
                    let right_cell = right.as_ref().map(|s| SplitDiffCell {
                        tag: match s.kind {
                            PatchLineKind::Added => DiffTag::Added,
                            _ => DiffTag::Context,
                        },
                        content: s.text.to_string(),
                        line_number: s.line_no,
                    });

                    let left_frame = split_render_cell(left_cell.as_ref(), left_content, lang_hint, context);
                    let right_frame = split_render_cell(right_cell.as_ref(), right_content, lang_hint, context);

                    let height = left_frame.lines().len().max(right_frame.lines().len());

                    let sep_line = Line::with_style(SEPARATOR.to_string(), sep_style);
                    let sep_frame = Frame::new(vec![sep_line; height]);
                    let row_frame = Frame::hstack([
                        FramePart::new(left_frame, left_panel_u16),
                        FramePart::new(sep_frame, SEPARATOR_WIDTH_U16),
                        FramePart::new(right_frame, right_panel_u16),
                    ]);

                    patch_lines.extend(row_frame.into_lines());
                    patch_refs.push(anchor);
                    patch_refs.extend(std::iter::repeat_n(None, height.saturating_sub(1)));
                }
            }

            if let Some(ref_key) = anchor
                && let Some(line_comments) = comment_map.get(&ref_key)
            {
                append_inline_comment_rows(&mut patch_lines, &mut patch_refs, line_comments, right_width, theme);
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
    Header { line_idx: usize, text: &'a str },
    Split { left: Option<SideInfo<'a>>, right: Option<SideInfo<'a>> },
}

fn pair_hunk_lines(lines: &[PatchLine]) -> Vec<PairedRow<'_>> {
    let mut rows = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let pl = &lines[i];
        match pl.kind {
            PatchLineKind::HunkHeader | PatchLineKind::Meta => {
                rows.push(PairedRow::Header { line_idx: i, text: &pl.text });
                i += 1;
            }
            PatchLineKind::Context => {
                rows.push(PairedRow::Split {
                    left: Some(SideInfo { kind: pl.kind, text: &pl.text, line_no: pl.old_line_no, line_idx: i }),
                    right: Some(SideInfo { kind: pl.kind, text: &pl.text, line_no: pl.new_line_no, line_idx: i }),
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

fn side_info(line: &PatchLine, line_idx: usize) -> SideInfo<'_> {
    let line_no = match line.kind {
        PatchLineKind::Added => line.new_line_no,
        PatchLineKind::Removed | PatchLineKind::Context => line.old_line_no,
        PatchLineKind::HunkHeader | PatchLineKind::Meta => None,
    };

    SideInfo { kind: line.kind, text: &line.text, line_no, line_idx }
}

fn pair_changed_block<'a>(removed: &[SideInfo<'a>], added: &[SideInfo<'a>]) -> Vec<PairedRow<'a>> {
    let old: Vec<&str> = removed.iter().map(|info| info.text).collect();
    let new: Vec<&str> = added.iter().map(|info| info.text).collect();
    let diff = TextDiff::from_slices(&old, &new);
    let mut rows = Vec::new();

    for op in diff.ops() {
        match *op {
            SimilarDiffOp::Equal { old_index, new_index, len } => {
                for offset in 0..len {
                    rows.push(split_row(Some(removed[old_index + offset]), Some(added[new_index + offset])));
                }
            }
            SimilarDiffOp::Delete { old_index, old_len, .. } => {
                for side in &removed[old_index..old_index + old_len] {
                    rows.push(split_row(Some(*side), None));
                }
            }
            SimilarDiffOp::Insert { new_index, new_len, .. } => {
                for side in &added[new_index..new_index + new_len] {
                    rows.push(split_row(None, Some(*side)));
                }
            }
            SimilarDiffOp::Replace { old_index, old_len, new_index, new_len } => {
                let pair_len = old_len.min(new_len);

                for offset in 0..pair_len {
                    let left = removed[old_index + offset];
                    let right = added[new_index + offset];
                    rows.push(split_row(Some(left), Some(right)));
                }

                for side in &removed[old_index + pair_len..old_index + old_len] {
                    rows.push(split_row(Some(*side), None));
                }
                for side in &added[new_index + pair_len..new_index + new_len] {
                    rows.push(split_row(None, Some(*side)));
                }
            }
        }
    }

    rows
}

fn split_row<'a>(left: Option<SideInfo<'a>>, right: Option<SideInfo<'a>>) -> PairedRow<'a> {
    PairedRow::Split { left, right }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_diff::{FileDiff, FileStatus, Hunk, PatchLine, PatchLineKind};

    fn pl(kind: PatchLineKind, text: &str, old: Option<usize>, new: Option<usize>) -> PatchLine {
        PatchLine { kind, text: text.to_string(), old_line_no: old, new_line_no: new }
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
        Hunk { header: "@@ -1,3 +1,3 @@".to_string(), old_start: 1, old_count: 3, new_start: 1, new_count: 3, lines }
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
        let (lines, refs) = build_split_patch_lines(&file, 100, &ctx(), &[]);
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
        let (lines, refs) = build_split_patch_lines(&file, 100, &ctx(), &[]);
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
        let (lines, _refs) = build_split_patch_lines(&file, 100, &ctx(), &[]);
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
        let (lines, refs) = build_split_patch_lines(&file, 100, &ctx(), &[]);
        let header_text = lines[0].plain_text();
        assert!(header_text.contains("@@ -1,1 +1,1 @@"), "header missing: {header_text}");
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
        let (_lines, refs) = build_split_patch_lines(&file, 100, &ctx(), &[]);
        // Layout: hunk1_header, context_a, spacer, hunk2_header, context_b
        assert_eq!(refs.len(), 5);
        assert!(refs[2].is_none(), "spacer between hunks should have None ref");
    }

    #[test]
    fn split_refs_prefer_right_side() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,1 +1,1 @@", None, None),
            pl(PatchLineKind::Removed, "old", Some(1), None),
            pl(PatchLineKind::Added, "new", None, Some(1)),
        ])]);
        let (_lines, refs) = build_split_patch_lines(&file, 100, &ctx(), &[]);
        // The paired row ref should point to the Added line (index 2 in the hunk)
        let paired_ref = refs[1].as_ref().unwrap();
        assert_eq!(paired_ref.line_index, 2, "should reference the Added line");
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
        let (lines, _refs) = build_split_patch_lines(&file, 100, &ctx(), &[]);

        let shared_row = lines.iter().skip(1).find(|line| line.plain_text().matches("shared_call();").count() == 2);

        assert!(shared_row.is_some(), "identical moved lines should be aligned onto the same split row");
    }

    #[test]
    fn replace_with_extra_added_line_keeps_prefix_aligned_and_overflow_unpaired() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,2 +1,3 @@", None, None),
            pl(PatchLineKind::Removed, "shared_call();", Some(1), None),
            pl(PatchLineKind::Removed, "let old_value = foo();", Some(2), None),
            pl(PatchLineKind::Added, "shared_call();", None, Some(1)),
            pl(PatchLineKind::Added, "let new_value = bar();", None, Some(2)),
            pl(PatchLineKind::Added, "extra_call();", None, Some(3)),
        ])]);
        let (lines, _refs) = build_split_patch_lines(&file, 100, &ctx(), &[]);

        let shared_row = lines.iter().skip(1).find(|line| line.plain_text().matches("shared_call();").count() == 2);
        assert!(shared_row.is_some(), "shared prefix line should stay aligned as an unchanged pair");

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

    #[test]
    fn split_inline_comment_renders_below_correct_row() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,1 +1,1 @@", None, None),
            pl(PatchLineKind::Removed, "old", Some(1), None),
            pl(PatchLineKind::Added, "new", None, Some(1)),
        ])]);
        let comments = vec![QueuedComment {
            file_path: "test.rs".to_string(),
            patch_ref: PatchLineRef { hunk_index: 0, line_index: 2 },
            line_text: "new".to_string(),
            line_number: Some(1),
            line_kind: PatchLineKind::Added,
            comment: "review comment".to_string(),
        }];
        let (lines, refs) = build_split_patch_lines(&file, 100, &ctx(), &comments);

        // Layout: header, paired row, then comment box
        assert!(lines.len() > 2, "should have comment rows, got {}", lines.len());

        let paired_row_text = lines[1].plain_text();
        assert!(paired_row_text.contains("old"), "paired row should contain 'old'");
        assert!(paired_row_text.contains("new"), "paired row should contain 'new'");

        let comment_content = lines[3].plain_text();
        assert!(comment_content.contains("review comment"), "should contain comment text, got: {comment_content}");

        // Comment rows should have None refs
        assert!(refs[2].is_none(), "comment top border should have None ref");
        assert!(refs[3].is_none(), "comment content should have None ref");
        assert!(refs[4].is_none(), "comment bottom border should have None ref");
    }

    #[test]
    fn split_multiple_comments_render_in_order() {
        let file = test_file(vec![test_hunk(vec![
            pl(PatchLineKind::HunkHeader, "@@ -1,1 +1,1 @@", None, None),
            pl(PatchLineKind::Added, "code", None, Some(1)),
        ])]);
        let comments = vec![
            QueuedComment {
                file_path: "test.rs".to_string(),
                patch_ref: PatchLineRef { hunk_index: 0, line_index: 1 },
                line_text: "code".to_string(),
                line_number: Some(1),
                line_kind: PatchLineKind::Added,
                comment: "alpha".to_string(),
            },
            QueuedComment {
                file_path: "test.rs".to_string(),
                patch_ref: PatchLineRef { hunk_index: 0, line_index: 1 },
                line_text: "code".to_string(),
                line_number: Some(1),
                line_kind: PatchLineKind::Added,
                comment: "beta".to_string(),
            },
        ];
        let (lines, _refs) = build_split_patch_lines(&file, 100, &ctx(), &comments);

        let text: Vec<String> = lines.iter().map(tui::Line::plain_text).collect();
        let alpha_pos = text.iter().position(|t| t.contains("alpha")).expect("should find alpha");
        let beta_pos = text.iter().position(|t| t.contains("beta")).expect("should find beta");
        assert!(alpha_pos < beta_pos, "alpha should appear before beta");
    }
}
