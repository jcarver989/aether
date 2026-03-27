use std::path::PathBuf;
use tui::Component;
use tui::ViewContext;
use tui::testing::{render_component, render_lines};
use wisp::components::app::{GitDiffLoadState, GitDiffViewState};
use wisp::components::git_diff_view::{GitDiffView, build_patch_lines};
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

fn make_view_state(doc: GitDiffDocument) -> GitDiffViewState {
    GitDiffViewState::new(GitDiffLoadState::Ready(doc))
}

#[test]
fn render_empty_state() {
    let mut state = GitDiffViewState::new(GitDiffLoadState::Empty);
    let mut view = GitDiffView { state: &mut state };
    let term = render_component(|ctx| view.render(ctx), 80, 24);
    let output = term.get_lines();
    let text = output.join("");
    assert!(text.contains("No changes"));
}

#[test]
fn render_error_state() {
    let mut state = GitDiffViewState::new(GitDiffLoadState::Error {
        message: "not a repo".to_string(),
    });
    let mut view = GitDiffView { state: &mut state };
    let term = render_component(|ctx| view.render(ctx), 80, 24);
    let output = term.get_lines();
    let text = output.join("");
    assert!(text.contains("Git diff unavailable"));
    assert!(text.contains("not a repo"));
}

#[test]
fn render_shows_file_list_and_patch() {
    let doc = make_test_doc();
    let mut state = make_view_state(doc);
    let mut view = GitDiffView { state: &mut state };
    let term = render_component(|ctx| view.render(ctx), 100, 24);
    let output = term.get_lines();
    assert!(!output.is_empty());

    let first_text = &output[0];
    assert!(
        first_text.contains("a.rs"),
        "Should show file name: {first_text}"
    );
}

#[test]
fn patch_lines_have_syntax_highlighted_spans() {
    let doc = make_test_doc();
    let context = ViewContext::new((100, 24));
    let file = &doc.files[0];
    let (patch_lines, _refs) = build_patch_lines(file, 100, &context);

    let term = render_lines(&patch_lines, 100, 24);
    let output = term.get_lines();

    // Context line "fn main() {" should contain code
    assert!(
        output[1].contains("fn main()"),
        "context line should contain code, got: {}",
        output[1]
    );

    // Added code spans should retain the diff background.
    let added_line = &patch_lines[3];
    assert!(
        added_line
            .spans()
            .iter()
            .skip(1)
            .any(|span| span.style().bg == Some(context.theme.diff_added_bg())),
        "added code spans should keep diff_added_bg"
    );
}
