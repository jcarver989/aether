use super::git_diff_comment_renderer::{self, DraftCommentState};
use super::patch_renderer::RenderedPatch;
use super::split_patch_renderer::build_split_patch_base_lines;
use crate::components::app::git_diff_mode::QueuedComment;
use crate::git_diff::FileDiff;
use tui::{Cursor, Frame, Line, Theme, ViewContext};

pub struct FrameSplice {
    pub after_row: usize,
    pub frame: Frame,
}

pub struct DraftSplice {
    pub splice: FrameSplice,
}

#[derive(Clone, PartialEq, Eq)]
struct DiffLayerKey {
    document_revision: usize,
    width: u16,
    file_path: String,
    layout: DiffLayout,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffLayout {
    Unified,
    Split,
}

pub struct CachedDiffLayer {
    key: Option<DiffLayerKey>,
    rendered: Option<RenderedPatch>,
}

impl CachedDiffLayer {
    fn new() -> Self {
        Self { key: None, rendered: None }
    }

    fn invalidate(&mut self) {
        self.key = None;
        self.rendered = None;
    }

    fn ensure(&mut self, file: &FileDiff, width: u16, split_layout: bool, document_revision: usize, ctx: &ViewContext) {
        let key = DiffLayerKey {
            document_revision,
            width,
            file_path: file.path.clone(),
            layout: if split_layout { DiffLayout::Split } else { DiffLayout::Unified },
        };

        if self.key.as_ref() == Some(&key) && self.rendered.is_some() {
            return;
        }

        self.rendered = Some(if split_layout {
            build_split_patch_base_lines(file, usize::from(width), ctx)
        } else {
            RenderedPatch::from_file_diff(file, usize::from(width), ctx)
        });
        self.key = Some(key);
    }
}

#[derive(Clone, PartialEq, Eq)]
struct CommentLayerKey {
    revision: usize,
    width: u16,
    file_path: String,
}

struct CachedCommentLayer {
    key: Option<CommentLayerKey>,
    splices: Vec<FrameSplice>,
}

impl CachedCommentLayer {
    fn new() -> Self {
        Self { key: None, splices: Vec::new() }
    }

    fn invalidate(&mut self) {
        self.key = None;
        self.splices.clear();
    }

    fn ensure(
        &mut self,
        file: &FileDiff,
        comments: &[&QueuedComment],
        width: u16,
        revision: usize,
        rendered: &RenderedPatch,
        theme: &Theme,
    ) {
        let key = CommentLayerKey { revision, width, file_path: file.path.clone() };
        if self.key.as_ref() == Some(&key) {
            return;
        }

        let diff_line_count = rendered.lines.len();
        let mut splices: Vec<FrameSplice> = Vec::new();

        for block in git_diff_comment_renderer::render_comment_blocks(comments, usize::from(width), theme) {
            let Some(insertion_row) = rendered.line_ref_to_anchor_row_index.get(&block.anchor).copied() else {
                continue;
            };
            if insertion_row == 0 || insertion_row > diff_line_count {
                continue;
            }

            let after_row = insertion_row - 1;
            if let Some(last) = splices.last_mut()
                && last.after_row == after_row
            {
                let mut lines = last.frame.lines().to_vec();
                lines.extend(block.rows);
                last.frame = Frame::new(lines);
            } else {
                splices.push(FrameSplice { after_row, frame: Frame::new(block.rows) });
            }
        }

        self.key = Some(key);
        self.splices = splices;
    }
}

pub struct GitDiffCompositor {
    diff_layer: CachedDiffLayer,
    submitted_layer: CachedCommentLayer,
    submitted_revision: usize,
}

impl GitDiffCompositor {
    pub fn new() -> Self {
        Self { diff_layer: CachedDiffLayer::new(), submitted_layer: CachedCommentLayer::new(), submitted_revision: 0 }
    }

    pub fn invalidate_diff_layer(&mut self) {
        self.diff_layer.invalidate();
    }

    pub fn invalidate_submitted_comments_layer(&mut self) {
        self.submitted_revision = self.submitted_revision.saturating_add(1);
        self.submitted_layer.invalidate();
    }

    pub fn invalidate_all(&mut self) {
        self.diff_layer.invalidate();
        self.invalidate_submitted_comments_layer();
    }

    pub fn ensure_diff_layer(
        &mut self,
        file: &FileDiff,
        width: u16,
        split_layout: bool,
        document_revision: usize,
        ctx: &ViewContext,
    ) {
        self.diff_layer.ensure(file, width, split_layout, document_revision, ctx);
    }

    pub fn ensure_submitted_layer(&mut self, file: &FileDiff, comments: &[&QueuedComment], width: u16, theme: &Theme) {
        let Some(rendered) = self.diff_layer.rendered.as_ref() else {
            self.submitted_layer.invalidate();
            return;
        };

        self.submitted_layer.ensure(file, comments, width, self.submitted_revision, rendered, theme);
    }

    pub fn rendered_patch(&self) -> Option<&RenderedPatch> {
        self.diff_layer.rendered.as_ref()
    }

    pub fn comment_splices(&self) -> &[FrameSplice] {
        &self.submitted_layer.splices
    }

    pub fn draft_splice(&self, draft: &DraftCommentState, width: usize, theme: &Theme) -> Option<DraftSplice> {
        let rendered = self.diff_layer.rendered.as_ref()?;
        let insertion_row = rendered.line_ref_to_anchor_row_index.get(&draft.anchor).copied()?;
        let block = git_diff_comment_renderer::render_draft_comment_block(draft, width, theme);

        let cursor_row = block.cursor_row_offset;
        let cursor_col = block.cursor_col.min(width.saturating_sub(1));
        let frame = Frame::new(block.block.rows).with_cursor(Cursor::visible(cursor_row, cursor_col));

        Some(DraftSplice { splice: FrameSplice { after_row: insertion_row - 1, frame } })
    }
}

impl Default for GitDiffCompositor {
    fn default() -> Self {
        Self::new()
    }
}

pub fn apply_cursor_highlight(line: &Line, theme: &Theme) -> Line {
    let highlight_bg = theme.highlight_bg();
    let mut highlighted = Line::default();

    for span in line.spans() {
        highlighted.push_with_style(span.text(), span.style().bg_color(highlight_bg));
    }

    if line.is_empty() {
        highlighted.push_with_style(" ", tui::Style::default().bg_color(highlight_bg));
    }

    highlighted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::app::git_diff_mode::PatchLineRef;
    use crate::git_diff::{FileStatus, Hunk, PatchLine, PatchLineKind};

    fn context() -> ViewContext {
        ViewContext::new((120, 24))
    }

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

    fn queued(anchor: PatchLineRef, comment: &str) -> QueuedComment {
        QueuedComment {
            file_path: "test.rs".to_string(),
            patch_ref: anchor,
            line_text: "line".to_string(),
            line_number: Some(1),
            line_kind: PatchLineKind::Added,
            comment: comment.to_string(),
        }
    }

    fn initialize_layers(
        compositor: &mut GitDiffCompositor,
        file: &FileDiff,
        width: u16,
        comments: &[QueuedComment],
        document_revision: usize,
    ) {
        let ctx = context();
        let refs = comments.iter().collect::<Vec<_>>();
        compositor.ensure_diff_layer(file, width, false, document_revision, &ctx);
        compositor.ensure_submitted_layer(file, &refs, width, &ctx.theme);
    }

    #[test]
    fn compositor_starts_empty() {
        let compositor = GitDiffCompositor::new();
        assert!(compositor.rendered_patch().is_none());
        assert!(compositor.comment_splices().is_empty());
    }

    #[test]
    fn comment_splices_preserve_input_order_for_same_anchor() {
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine {
                kind: PatchLineKind::Added,
                text: "new_line();".to_string(),
                old_line_no: None,
                new_line_no: Some(1),
            },
        ]);

        let anchor = PatchLineRef { hunk_index: 0, line_index: 1 };
        let comments = vec![queued(anchor, "alpha"), queued(anchor, "beta")];

        let mut compositor = GitDiffCompositor::new();
        initialize_layers(&mut compositor, &file, 80, &comments, 1);

        let splices = compositor.comment_splices();
        assert_eq!(splices.len(), 1);
        let rendered_text: Vec<String> = splices[0].frame.lines().iter().map(Line::plain_text).collect();
        let alpha_pos = rendered_text.iter().position(|t| t.contains("alpha")).expect("alpha should render");
        let beta_pos = rendered_text.iter().position(|t| t.contains("beta")).expect("beta should render");
        assert!(alpha_pos < beta_pos, "comments should render in queue order");
    }

    #[test]
    fn comment_splice_uses_correct_after_row() {
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine {
                kind: PatchLineKind::Added,
                text: "new_line();".to_string(),
                old_line_no: None,
                new_line_no: Some(1),
            },
        ]);

        let anchor = PatchLineRef { hunk_index: 0, line_index: 1 };
        let comments = vec![queued(anchor, "a comment")];

        let mut compositor = GitDiffCompositor::new();
        initialize_layers(&mut compositor, &file, 80, &comments, 1);

        let splices = compositor.comment_splices();
        assert_eq!(splices.len(), 1);

        let rendered = compositor.rendered_patch().unwrap();
        let insertion_row = rendered.line_ref_to_anchor_row_index[&anchor];
        assert_eq!(splices[0].after_row, insertion_row - 1);
    }

    #[test]
    fn draft_splice_positions_cursor() {
        let file = make_file(vec![
            PatchLine {
                kind: PatchLineKind::HunkHeader,
                text: "@@ -1,1 +1,1 @@".to_string(),
                old_line_no: None,
                new_line_no: None,
            },
            PatchLine {
                kind: PatchLineKind::Added,
                text: "new_line();".to_string(),
                old_line_no: None,
                new_line_no: Some(1),
            },
        ]);

        let mut compositor = GitDiffCompositor::new();
        initialize_layers(&mut compositor, &file, 40, &[], 1);

        let draft = DraftCommentState {
            anchor: PatchLineRef { hunk_index: 0, line_index: 1 },
            text: "hello".to_string(),
            cursor_position: 5,
        };

        let splice = compositor.draft_splice(&draft, 40, &context().theme).expect("draft splice should exist");
        assert!(splice.splice.frame.cursor().is_visible);
        assert!(splice.splice.frame.lines().len() >= 3);
    }

    #[test]
    fn draft_splice_returns_none_for_unknown_anchor() {
        let file = make_file(vec![PatchLine {
            kind: PatchLineKind::HunkHeader,
            text: "@@ -1,1 +1,1 @@".to_string(),
            old_line_no: None,
            new_line_no: None,
        }]);

        let mut compositor = GitDiffCompositor::new();
        initialize_layers(&mut compositor, &file, 40, &[], 1);

        let draft = DraftCommentState {
            anchor: PatchLineRef { hunk_index: 99, line_index: 99 },
            text: "hello".to_string(),
            cursor_position: 0,
        };

        assert!(compositor.draft_splice(&draft, 40, &context().theme).is_none());
    }
}
