use std::path::PathBuf;
use tui::ViewContext;
use tui::testing::{assert_buffer_eq, cols, render_component, render_lines};
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
