use std::path::PathBuf;
use tui::testing::{assert_buffer_eq, cols, key, render_component, render_lines};
use tui::{Component, Event, KeyCode, MIN_GUTTER_WIDTH, SEPARATOR_WIDTH, ViewContext};
use wisp::components::app::{GitDiffLoadState, GitDiffMode};
use wisp::git_diff::{FileDiff, FileStatus, GitDiffDocument, Hunk, PatchLine, PatchLineKind};

const HINT_LINE: &str = "j/k:move  n/p:hunk  h/l:focus  c:comment  s:submit  u:undo  r:refresh  Esc:close";

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
    let term = render_component(|ctx| mode.render(ctx), 140, 12);
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

    let ctx = ViewContext::new((140, 12));
    let added_bg = Some(ctx.theme.diff_added_bg());
    let removed_bg = Some(ctx.theme.diff_removed_bg());
    let separator_and_gutter = SEPARATOR_WIDTH + MIN_GUTTER_WIDTH;
    let separator_start = right_start.saturating_sub(separator_and_gutter);
    assert!(separator_start > 0, "first row's RIGHT_HEAD should be preceded by separator + gutter columns");
    for col in separator_start..right_start {
        let actual_bg = term.get_style_at(wrapped_idx, col).bg;
        assert_ne!(actual_bg, added_bg, "separator/gutter column {col} should not inherit added background");
        assert_ne!(actual_bg, removed_bg, "separator/gutter column {col} should not inherit removed background");
    }
}

#[test]
fn wrapped_split_diff_continuation_row_keeps_neutral_padding() {
    let mut mode = make_mode(make_wrapping_split_doc());
    let ctx = ViewContext::new((140, 12));
    let frame = mode.render(&ctx);
    let wrapped_row = frame
        .lines()
        .iter()
        .find(|line| line.plain_text().contains("RIGHT_TAIL"))
        .cloned()
        .expect("expected wrapped continuation row containing RIGHT_TAIL");

    let term = render_lines(&[wrapped_row], 140, 1);

    // The continuation row's LEFT pane is blank (no bg), then SEP, then RIGHT
    // pane's tail-gutter (blank, no bg), then RIGHT bg-padded content. Verify
    // the SEP + RIGHT-gutter columns immediately before the RIGHT content
    // start carry no diff background.
    let added_bg = Some(ctx.theme.diff_added_bg());
    let removed_bg = Some(ctx.theme.diff_removed_bg());
    let right_content_start = term
        .get_lines()
        .first()
        .and_then(|line| (0..line.len()).find(|&col| term.get_style_at(0, col).bg == added_bg))
        .expect("wrapped row should contain at least one cell with the diff_added bg");
    let neutral_start = right_content_start.saturating_sub(SEPARATOR_WIDTH + MIN_GUTTER_WIDTH);
    for col in neutral_start..right_content_start {
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
    let term = render_component(|ctx| mode.render(ctx), 140, 7);

    assert_buffer_eq(
        &term,
        &[
            cols(&[(">   M x.rs             +1/-1", 28), ("", 1), ("x.rs  (modified)", 0)]),
            String::new(),
            cols(&[("", 28), ("", 1), ("@@ -1,2 +1,2 @@", 0)]),
            cols(&[("", 29), (" 1 LEFT_MARK", 55), ("", 1), (" 1 RIGHT_HEAD", 55)]),
            cols(&[("", 29), ("", 55), ("", 1), ("", 3), (filler.as_str(), 0)]),
            cols(&[("", 29), ("", 55), ("", 1), ("", 3), ("RIGHT_TAIL", 0)]),
            HINT_LINE.to_string(),
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
    let term = render_component(|ctx| mode.render(ctx), 151, 8);
    let lines = term.get_lines();
    let wrapped_idx = lines
        .iter()
        .position(|line| line.contains("blank_panel(left_panel));") && line.contains("theme.code_bg()));"))
        .expect("expected wrapped row containing both continuation segments");
    let wrapped_row = &lines[wrapped_idx];

    assert_buffer_eq(
        &render_lines(&[tui::Line::new(wrapped_row.clone())], 151, 1),
        &[cols(&[
            ("", 32),
            ("blank_panel(left_panel));", 58),
            ("", 4),
            ("blank_panel(left_panel, theme.code_bg()));", 0),
        ])],
    );

    let left_start = wrapped_row.find("blank_panel(left_panel));").expect("expected wrapped removed continuation");
    let right_start =
        wrapped_row.find("blank_panel(left_panel, theme.code_bg()));").expect("expected wrapped added continuation");

    let ctx = ViewContext::new((151, 8));
    let added_bg = Some(ctx.theme.diff_added_bg());
    let removed_bg = Some(ctx.theme.diff_removed_bg());
    let code_panel_start = left_start.saturating_sub(MIN_GUTTER_WIDTH);
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
    let term = render_component(|ctx| mode.render(ctx), 80, 3);
    assert_buffer_eq(
        &term,
        &[
            cols(&[("", sb), ("", 1), ("No changes in working tree relative to HEAD", 0)]),
            String::new(),
            HINT_LINE.to_string(),
        ],
    );
}

#[test]
fn render_error_state() {
    let sb = 26;
    let mut mode = GitDiffMode::new(PathBuf::from("."));
    mode.set_load_state(GitDiffLoadState::Error { message: "not a repo".to_string() });
    let term = render_component(|ctx| mode.render(ctx), 80, 3);
    assert_buffer_eq(
        &term,
        &[cols(&[("", sb), ("", 1), ("Git diff unavailable: not a repo", 0)]), String::new(), HINT_LINE.to_string()],
    );
}

#[test]
fn render_shows_file_list_and_patch() {
    let sb = 28;
    let doc = make_test_doc();
    let mut mode = make_mode(doc);
    let term = render_component(|ctx| mode.render(ctx), 100, 9);
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
            HINT_LINE.to_string(),
        ],
    );
}

#[test]
fn added_lines_use_added_background_style() {
    let mut mode = make_mode(make_test_doc());
    let term = render_component(|ctx| mode.render(ctx), 100, 8);
    let lines = term.get_lines();

    let added_row = lines.iter().position(|line| line.contains("new();")).expect("expected added diff line");
    let added_col = lines[added_row].find("new();").expect("expected added code text in row");

    let ctx = ViewContext::new((100, 8));
    assert_eq!(term.get_style_at(added_row, added_col).bg, Some(ctx.theme.diff_added_bg()));
}

#[test]
fn narrow_width_renders_unified_diff_rows() {
    let mut mode = make_mode(make_test_doc());
    let term = render_component(|ctx| mode.render(ctx), 108, 10);
    let lines = term.get_lines();

    assert!(lines.iter().any(|line| line.contains("old();")), "expected removed line in unified view");
    assert!(lines.iter().any(|line| line.contains("new();")), "expected added line in unified view");
    assert!(
        !lines.iter().any(|line| line.contains("old();") && line.contains("new();")),
        "unified view should keep old/new content on separate rows"
    );
}

#[test]
fn wide_width_renders_split_diff_rows() {
    let mut mode = make_mode(make_test_doc());
    let term = render_component(|ctx| mode.render(ctx), 109, 10);
    let lines = term.get_lines();

    assert!(
        lines.iter().any(|line| line.contains("old();") && line.contains("new();")),
        "split view should render old/new content on the same row"
    );
}

#[test]
fn git_diff_mode_soft_wraps_long_patch_headers_in_rhs_panel() {
    let mut mode = make_mode(make_long_header_doc());
    let term = render_component(|ctx| mode.render(ctx), 100, 8);
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
    let term = render_component(|ctx| mode.render(ctx), 130, 10);
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

fn make_comment_test_doc() -> GitDiffDocument {
    GitDiffDocument {
        repo_root: PathBuf::from("/tmp/test"),
        files: vec![FileDiff {
            old_path: Some("test.rs".to_string()),
            path: "test.rs".to_string(),
            status: FileStatus::Added,
            hunks: vec![Hunk {
                header: "@@ -0,0 +1,3 @@".to_string(),
                old_start: 0,
                old_count: 0,
                new_start: 1,
                new_count: 3,
                lines: vec![
                    PatchLine {
                        kind: PatchLineKind::HunkHeader,
                        text: "@@ -0,0 +1,3 @@".to_string(),
                        old_line_no: None,
                        new_line_no: None,
                    },
                    PatchLine {
                        kind: PatchLineKind::Added,
                        text: "line_one".to_string(),
                        old_line_no: None,
                        new_line_no: Some(1),
                    },
                    PatchLine {
                        kind: PatchLineKind::Added,
                        text: "line_two".to_string(),
                        old_line_no: None,
                        new_line_no: Some(2),
                    },
                    PatchLine {
                        kind: PatchLineKind::Added,
                        text: "line_three".to_string(),
                        old_line_no: None,
                        new_line_no: Some(3),
                    },
                ],
            }],
            binary: false,
        }],
    }
}

async fn send_keys(mode: &mut GitDiffMode, codes: &[KeyCode]) {
    let ctx = ViewContext::new((100, 20));
    for &code in codes {
        mode.render(&ctx);
        mode.on_event(&Event::Key(key(code))).await;
    }
}

#[tokio::test]
async fn draft_comment_appears_after_correct_line_when_submitted_comment_exists() {
    let mut mode = make_mode(make_comment_test_doc());

    // Focus right panel (l on file list triggers FileOpened)
    send_keys(&mut mode, &[KeyCode::Char('l')]).await;
    // Move cursor down to line_one, open comment, type "first", submit
    send_keys(
        &mut mode,
        &[
            KeyCode::Char('j'),
            KeyCode::Char('c'),
            KeyCode::Char('f'),
            KeyCode::Char('i'),
            KeyCode::Char('r'),
            KeyCode::Char('s'),
            KeyCode::Char('t'),
            KeyCode::Enter,
        ],
    )
    .await;
    // Move cursor to line_three (two j presses), open draft, type "draft"
    send_keys(
        &mut mode,
        &[
            KeyCode::Char('j'),
            KeyCode::Char('j'),
            KeyCode::Char('c'),
            KeyCode::Char('d'),
            KeyCode::Char('r'),
            KeyCode::Char('a'),
            KeyCode::Char('f'),
            KeyCode::Char('t'),
        ],
    )
    .await;

    let term = render_component(|ctx| mode.render(ctx), 100, 20);
    let lines = term.get_lines();

    let line_one_row = lines.iter().position(|l| l.contains("line_one")).expect("line_one should render");
    let comment_row = lines.iter().position(|l| l.contains("first")).expect("submitted comment should render");
    let line_two_row = lines.iter().position(|l| l.contains("line_two")).expect("line_two should render");
    let line_three_row = lines.iter().position(|l| l.contains("line_three")).expect("line_three should render");
    let draft_row = lines.iter().position(|l| l.contains("draft")).expect("draft text should render");

    assert!(
        comment_row > line_one_row,
        "submitted comment (row {comment_row}) should appear after line_one (row {line_one_row})"
    );
    assert!(
        line_two_row > comment_row,
        "line_two (row {line_two_row}) should appear after submitted comment (row {comment_row})"
    );
    assert!(
        line_three_row > line_two_row,
        "line_three (row {line_three_row}) should appear after line_two (row {line_two_row})"
    );
    assert!(
        draft_row > line_three_row,
        "draft (row {draft_row}) should appear after line_three (row {line_three_row}), \
         not shifted up by the submitted comment splice"
    );
}

#[tokio::test]
async fn submitted_comment_visible_on_last_line() {
    let mut mode = make_mode(make_comment_test_doc());

    send_keys(&mut mode, &[KeyCode::Char('l')]).await;
    send_keys(
        &mut mode,
        &[
            KeyCode::Char('j'),
            KeyCode::Char('j'),
            KeyCode::Char('j'),
            KeyCode::Char('c'),
            KeyCode::Char('h'),
            KeyCode::Char('i'),
            KeyCode::Enter,
        ],
    )
    .await;

    // height=7 → split-body height=4, exactly fits 4 diff lines.
    // The 3-row comment box below line_three is entirely off-screen without a scroll fix.
    let term = render_component(|ctx| mode.render(ctx), 100, 7);
    let lines = term.get_lines();

    assert!(lines.iter().any(|l| l.contains("line_three")), "cursor line should be visible, got: {lines:?}");
    assert!(
        lines.iter().any(|l| l.contains("hi")),
        "submitted comment text should be visible in viewport, got: {lines:?}"
    );
    assert!(lines.iter().any(|l| l.contains("└")), "comment bottom border should be visible, got: {lines:?}");
}

#[tokio::test]
async fn draft_comment_bottom_border_visible_on_last_line() {
    let mut mode = make_mode(make_comment_test_doc());

    send_keys(&mut mode, &[KeyCode::Char('l')]).await;
    send_keys(
        &mut mode,
        &[
            KeyCode::Char('j'),
            KeyCode::Char('j'),
            KeyCode::Char('j'),
            KeyCode::Char('c'),
            KeyCode::Char('h'),
            KeyCode::Char('i'),
        ],
    )
    .await;

    // height=8 → body_height=6. 4 diff lines + 3 draft rows = 7 body rows.
    // Without fix the draft content row is visible but the bottom border is clipped.
    let term = render_component(|ctx| mode.render(ctx), 100, 8);
    let lines = term.get_lines();

    assert!(lines.iter().any(|l| l.contains("hi")), "draft text should be visible, got: {lines:?}");
    assert!(lines.iter().any(|l| l.contains("└")), "draft bottom border should be visible, got: {lines:?}");
}
