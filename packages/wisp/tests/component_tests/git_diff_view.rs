use std::path::PathBuf;
use tui::testing::{assert_buffer_eq, cols, render_component, render_lines};
use tui::{GUTTER_WIDTH, SEPARATOR_WIDTH, ViewContext};
use wisp::components::app::{GitDiffLoadState, GitDiffMode};
use wisp::components::patch_renderer::build_patch_lines;
use wisp::git_diff::{FileDiff, FileStatus, GitDiffDocument, Hunk, PatchLine, PatchLineKind};

fn make_test_doc() -> GitDiffDocument {
    GitDiffDocument {
        repo_root: PathBuf::from("/tmp/test"),
        files: vec![
            FileDiff {
                old_path: Some("a.rs".to_string()),
                path: "a.rs".to_string(),
                status: FileStatus::Modified,
                hunks: vec![Hunk {
                    header: "@@ -1,3 +1,3 @@".to_string(),
                    old_start: 1,
                    old_count: 3,
                    new_start: 1,
                    new_count: 3,
                    lines: vec![
                        PatchLine {
                            kind: PatchLineKind::HunkHeader,
                            text: "@@ -1,3 +1,3 @@".to_string(),
                            old_line_no: None,
                            new_line_no: None,
                        },
                        PatchLine {
                            kind: PatchLineKind::Context,
                            text: "fn main() {".to_string(),
                            old_line_no: Some(1),
                            new_line_no: Some(1),
                        },
                        PatchLine {
                            kind: PatchLineKind::Removed,
                            text: "    old();".to_string(),
                            old_line_no: Some(2),
                            new_line_no: None,
                        },
                        PatchLine {
                            kind: PatchLineKind::Added,
                            text: "    new();".to_string(),
                            old_line_no: None,
                            new_line_no: Some(2),
                        },
                        PatchLine {
                            kind: PatchLineKind::Context,
                            text: "}".to_string(),
                            old_line_no: Some(3),
                            new_line_no: Some(3),
                        },
                    ],
                }],
                binary: false,
            },
            FileDiff {
                old_path: None,
                path: "b.rs".to_string(),
                status: FileStatus::Added,
                hunks: vec![Hunk {
                    header: "@@ -0,0 +1,1 @@".to_string(),
                    old_start: 0,
                    old_count: 0,
                    new_start: 1,
                    new_count: 1,
                    lines: vec![
                        PatchLine {
                            kind: PatchLineKind::HunkHeader,
                            text: "@@ -0,0 +1,1 @@".to_string(),
                            old_line_no: None,
                            new_line_no: None,
                        },
                        PatchLine {
                            kind: PatchLineKind::Added,
                            text: "new_content".to_string(),
                            old_line_no: None,
                            new_line_no: Some(1),
                        },
                    ],
                }],
                binary: false,
            },
        ],
    }
}

fn make_mode(doc: GitDiffDocument) -> GitDiffMode {
    let mut mode = GitDiffMode::new(PathBuf::from("."));
    mode.load_document(doc);
    mode
}

fn make_wrapping_split_doc() -> GitDiffDocument {
    GitDiffDocument {
        repo_root: PathBuf::from("/tmp/test"),
        files: vec![FileDiff {
            old_path: Some("x.rs".to_string()),
            path: "x.rs".to_string(),
            status: FileStatus::Modified,
            hunks: vec![Hunk {
                header: "@@ -1,2 +1,2 @@".to_string(),
                old_start: 1,
                old_count: 2,
                new_start: 1,
                new_count: 2,
                lines: vec![
                    PatchLine {
                        kind: PatchLineKind::HunkHeader,
                        text: "@@ -1,2 +1,2 @@".to_string(),
                        old_line_no: None,
                        new_line_no: None,
                    },
                    PatchLine {
                        kind: PatchLineKind::Removed,
                        text: "LEFT_MARK".to_string(),
                        old_line_no: Some(1),
                        new_line_no: None,
                    },
                    PatchLine {
                        kind: PatchLineKind::Added,
                        text: format!("RIGHT_HEAD {} RIGHT_TAIL", "A".repeat(140)),
                        old_line_no: None,
                        new_line_no: Some(1),
                    },
                    PatchLine {
                        kind: PatchLineKind::Context,
                        text: "}".to_string(),
                        old_line_no: Some(2),
                        new_line_no: Some(2),
                    },
                ],
            }],
            binary: false,
        }],
    }
}

#[test]
fn wrapped_right_pane_rows_keep_a_neutral_boundary() {
    let mut mode = make_mode(make_wrapping_split_doc());
    let term = render_component(|ctx| mode.render_frame(ctx), 140, 12);
    let lines = term.get_lines();

    let first_row = lines
        .iter()
        .position(|line| line.contains("LEFT_MARK") && line.contains("RIGHT_HEAD"))
        .expect("expected split row containing both left and right markers");

    let right_start = lines[first_row].find("RIGHT_HEAD").expect("expected RIGHT_HEAD marker in first row");

    let wrapped_idx = lines
        .iter()
        .enumerate()
        .skip(first_row + 1)
        .find_map(|(index, line)| line.contains("RIGHT_TAIL").then_some(index))
        .expect("expected wrapped continuation row containing RIGHT_TAIL marker");

    let wrapped_start = lines[wrapped_idx].find("RIGHT_TAIL").expect("expected RIGHT_TAIL marker in wrapped row");

    assert!(
        wrapped_start >= right_start,
        "wrapped continuation should not start left of original right-pane content start (was {wrapped_start}, expected >= {right_start})"
    );

    let padding_width = GUTTER_WIDTH + SEPARATOR_WIDTH;
    assert!(wrapped_start >= padding_width, "wrapped content should leave room for separator and gutter");
    let ctx = ViewContext::new((140, 12));
    let added_bg = Some(ctx.theme.diff_added_bg());
    let removed_bg = Some(ctx.theme.diff_removed_bg());
    for col in (wrapped_start - padding_width)..wrapped_start {
        let actual_bg = term.get_style_at(wrapped_idx, col).bg;
        assert_ne!(actual_bg, added_bg, "padding column {col} should not inherit added background");
        assert_ne!(actual_bg, removed_bg, "padding column {col} should not inherit removed background");
    }
}

#[test]
fn wrapped_split_diff_continuation_row_keeps_neutral_padding() {
    let mut mode = make_mode(make_wrapping_split_doc());
    let ctx = ViewContext::new((140, 12));
    let frame = mode.render_frame(&ctx);
    let wrapped_row = frame
        .lines()
        .iter()
        .find(|line| line.plain_text().contains("RIGHT_TAIL"))
        .cloned()
        .expect("expected wrapped continuation row containing RIGHT_TAIL");

    let term = render_lines(&[wrapped_row], 140, 1);
    assert_buffer_eq(&term, &[cols(&[("", 91), ("RIGHT_TAIL", 0)])]);

    let added_bg = Some(ctx.theme.diff_added_bg());
    let removed_bg = Some(ctx.theme.diff_removed_bg());
    for col in 83..91 {
        let actual_bg = term.get_style_at(0, col).bg;
        assert_ne!(actual_bg, added_bg, "padding column {col} should not inherit added background");
        assert_ne!(actual_bg, removed_bg, "padding column {col} should not inherit removed background");
    }
}

#[test]
fn git_diff_view_keeps_wrapped_code_out_of_the_line_number_gutter() {
    let filler = "A".repeat(48);
    let mut mode = make_mode(GitDiffDocument {
        repo_root: PathBuf::from("/tmp/test"),
        files: vec![FileDiff {
            old_path: Some("x.rs".to_string()),
            path: "x.rs".to_string(),
            status: FileStatus::Modified,
            hunks: vec![Hunk {
                header: "@@ -1,2 +1,2 @@".to_string(),
                old_start: 1,
                old_count: 2,
                new_start: 1,
                new_count: 2,
                lines: vec![
                    PatchLine {
                        kind: PatchLineKind::HunkHeader,
                        text: "@@ -1,2 +1,2 @@".to_string(),
                        old_line_no: None,
                        new_line_no: None,
                    },
                    PatchLine {
                        kind: PatchLineKind::Removed,
                        text: "LEFT_MARK".to_string(),
                        old_line_no: Some(1),
                        new_line_no: None,
                    },
                    PatchLine {
                        kind: PatchLineKind::Added,
                        text: format!("RIGHT_HEAD {filler} RIGHT_TAIL"),
                        old_line_no: None,
                        new_line_no: Some(1),
                    },
                ],
            }],
            binary: false,
        }],
    });
    let term = render_component(|ctx| mode.render_frame(ctx), 140, 6);

    assert_buffer_eq(
        &term,
        &[
            cols(&[(">   M x.rs             +1/-1", 28), ("", 1), ("x.rs  (modified)", 0)]),
            String::new(),
            cols(&[("", 28), ("", 1), ("@@ -1,2 +1,2 @@", 0)]),
            cols(&[("", 29), ("   1 LEFT_MARK", 54), ("", 3), ("   1 RIGHT_HEAD", 54)]),
            cols(&[("", 29), ("", 54), ("", 3), ("", 5), (filler.as_str(), 0)]),
            cols(&[("", 29), ("", 54), ("", 3), ("", 5), ("RIGHT_TAIL", 0)]),
        ],
    );
}

#[test]
fn screenshot_shaped_git_diff_wrap_row_stays_out_of_gutters() {
    let mut mode = make_mode(GitDiffDocument {
        repo_root: PathBuf::from("/tmp/test"),
        files: vec![FileDiff {
            old_path: Some("split_diff.rs".to_string()),
            path: "split_diff.rs".to_string(),
            status: FileStatus::Modified,
            hunks: vec![Hunk {
                header: "@@ -56,2 +57,2 @@".to_string(),
                old_start: 56,
                old_count: 2,
                new_start: 57,
                new_count: 2,
                lines: vec![
                    PatchLine {
                        kind: PatchLineKind::HunkHeader,
                        text: "@@ -56,2 +57,2 @@".to_string(),
                        old_line_no: None,
                        new_line_no: None,
                    },
                    PatchLine {
                        kind: PatchLineKind::Removed,
                        text: "let left = left_lines.get(i).cloned().unwrap_or_else(|| blank_panel(left_panel));"
                            .to_string(),
                        old_line_no: Some(56),
                        new_line_no: None,
                    },
                    PatchLine {
                        kind: PatchLineKind::Added,
                        text: "let left = left_lines.get(i).cloned().unwrap_or_else(|| blank_panel(left_panel, theme.code_bg()));"
                            .to_string(),
                        old_line_no: None,
                        new_line_no: Some(57),
                    },
                ],
            }],
            binary: false,
        }],
    });
    let term = render_component(|ctx| mode.render_frame(ctx), 151, 8);
    let lines = term.get_lines();
    let wrapped_idx = lines
        .iter()
        .position(|line| line.contains("blank_panel(left_panel));") && line.contains("theme.code_bg()));"))
        .expect("expected wrapped row containing both continuation segments");
    let wrapped_row = &lines[wrapped_idx];

    assert_buffer_eq(
        &render_lines(&[tui::Line::new(wrapped_row.clone())], 151, 1),
        &[cols(&[("", 34), ("| blank_panel(left_panel));", 62), ("blank_panel(left_panel, theme.code_bg()));", 0)])],
    );

    let left_start = wrapped_row.find("| blank_panel(left_panel));").expect("expected wrapped removed continuation");
    let right_start =
        wrapped_row.find("blank_panel(left_panel, theme.code_bg()));").expect("expected wrapped added continuation");

    let ctx = ViewContext::new((151, 8));
    let added_bg = Some(ctx.theme.diff_added_bg());
    let removed_bg = Some(ctx.theme.diff_removed_bg());
    let code_panel_start = left_start.saturating_sub(GUTTER_WIDTH);
    for col in code_panel_start..left_start {
        let actual_bg = term.get_style_at(wrapped_idx, col).bg;
        assert_ne!(actual_bg, added_bg, "blank left panel column {col} should not inherit added background");
        assert_ne!(actual_bg, removed_bg, "blank left panel column {col} should not inherit removed background");
    }
    assert_eq!(term.get_style_at(wrapped_idx, left_start).bg, Some(ctx.theme.diff_removed_bg()));
    assert_eq!(term.get_style_at(wrapped_idx, right_start).bg, Some(ctx.theme.diff_added_bg()));
}

fn make_long_header_doc() -> GitDiffDocument {
    let mut doc = make_test_doc();
    let long_path = "src/components/git_diff_mode/this_is_a_deliberately_long_filename_that_should_be_clipped_in_the_patch_header.rs".to_string();
    doc.files[0].old_path = Some(long_path.clone());
    doc.files[0].path = long_path;
    doc
}

fn make_long_split_hunk_header_doc() -> GitDiffDocument {
    let mut doc = make_test_doc();
    let long_header = format!("@@ -1,3 +1,3 @@ {}", "WRAPME_".repeat(30));
    doc.files[0].hunks[0].header.clone_from(&long_header);
    doc.files[0].hunks[0].lines[0].text = long_header;
    doc
}

#[test]
fn render_empty_state() {
    let sb = 26;
    let mut mode = GitDiffMode::new(PathBuf::from("."));
    let term = render_component(|ctx| mode.render_frame(ctx), 80, 3);
    assert_buffer_eq(
        &term,
        &[cols(&[("", sb), ("", 1), ("No changes in working tree relative to HEAD", 0)]), String::new(), String::new()],
    );
}

#[test]
fn render_error_state() {
    let sb = 26;
    let mut mode = GitDiffMode::new(PathBuf::from("."));
    mode.load_state = GitDiffLoadState::Error { message: "not a repo".to_string() };
    let term = render_component(|ctx| mode.render_frame(ctx), 80, 3);
    assert_buffer_eq(
        &term,
        &[cols(&[("", sb), ("", 1), ("Git diff unavailable: not a repo", 0)]), String::new(), String::new()],
    );
}

#[test]
fn render_shows_file_list_and_patch() {
    let sb = 28;
    let doc = make_test_doc();
    let mut mode = make_mode(doc);
    let term = render_component(|ctx| mode.render_frame(ctx), 100, 8);
    assert_buffer_eq(
        &term,
        &[
            cols(&[(">   M a.rs             +1/-1", sb), ("", 1), ("a.rs  (modified)", 0)]),
            cols(&[("    A b.rs             +1/-0", sb), ("", 1)]),
            cols(&[("", sb), ("", 1), ("@@ -1,3 +1,3 @@", 0)]),
            cols(&[("", sb), ("", 1), ("1 1   fn main() {", 0)]),
            cols(&[("", sb), ("", 1), ("2   -     old();", 0)]),
            cols(&[("", sb), ("", 1), ("  2 +     new();", 0)]),
            cols(&[("", sb), ("", 1), ("3 3   }", 0)]),
            String::new(),
        ],
    );
}

#[test]
fn patch_lines_have_syntax_highlighted_spans() {
    let doc = make_test_doc();
    let context = ViewContext::new((100, 24));
    let file = &doc.files[0];
    let (patch_lines, _refs) = build_patch_lines(file, 100, &context, &[]);

    let term = render_lines(&patch_lines, 100, 24);
    let output = term.get_lines();

    assert!(output[1].contains("fn main()"), "context line should contain code, got: {}", output[1]);

    let added_line = &patch_lines[3];
    assert!(
        added_line.spans().iter().skip(1).any(|span| span.style().bg == Some(context.theme.diff_added_bg())),
        "added code spans should keep diff_added_bg"
    );
}

#[test]
fn git_diff_mode_soft_wraps_long_patch_headers_in_rhs_panel() {
    let mut mode = make_mode(make_long_header_doc());
    let term = render_component(|ctx| mode.render_frame(ctx), 100, 8);
    let lines = term.get_lines();

    assert!(
        lines.iter().any(|line| line.contains("this_is_a_deliberately_long_filename")),
        "expected a line containing the start of the long header, got {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("should_be_clipped_in_the_patch_header.rs")),
        "expected a line containing the wrapped tail of the long header, got {lines:?}"
    );
    assert!(lines.iter().all(|line| line.chars().count() <= 100));
}

#[test]
fn git_split_view_preserves_hunk_header_background_on_wrapped_rows() {
    let mut mode = make_mode(make_long_split_hunk_header_doc());
    let term = render_component(|ctx| mode.render_frame(ctx), 130, 10);
    let lines = term.get_lines();

    let header_row = lines
        .iter()
        .position(|line| line.contains("@@ -1,3 +1,3 @@"))
        .expect("expected hunk header row to be rendered");
    let header_col = lines[header_row].find("@@ -1,3 +1,3 @@").expect("expected hunk header text in row");

    assert!(
        lines.get(header_row + 1).is_some_and(|line| line.contains("WRAPME_")),
        "expected wrapped hunk header continuation row, got {lines:?}"
    );

    let expected_bg = term.get_style_at(header_row, header_col).bg;
    assert!(expected_bg.is_some(), "expected hunk header to have background style");
    assert_eq!(term.get_style_at(header_row + 1, header_col).bg, expected_bg);
    assert_eq!(term.get_style_at(header_row + 1, 129).bg, expected_bg);
}
