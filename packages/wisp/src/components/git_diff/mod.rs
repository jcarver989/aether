pub(crate) mod git_diff_compositor;
pub(crate) mod git_diff_panel;
pub(crate) mod patch_renderer;
pub(crate) mod split_patch_renderer;

use crate::components::review_comments::CommentAnchor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PatchAnchor {
    pub hunk: usize,
    pub line: usize,
}

pub(crate) type DiffAnchor = CommentAnchor<PatchAnchor>;
