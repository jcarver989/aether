/// Tag indicating the kind of change a diff line represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffTag {
    Context,
    Removed,
    Added,
}

/// A single line in a diff, tagged with its change type.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffLine {
    pub tag: DiffTag,
    pub content: String,
}

/// A row in a side-by-side diff, pairing an old (left) line with a new (right) line.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitDiffRow {
    pub left: Option<SplitDiffCell>,
    pub right: Option<SplitDiffCell>,
}

/// One side of a split diff row.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitDiffCell {
    pub tag: DiffTag,
    pub content: String,
    pub line_number: Option<usize>,
    /// Byte offset ranges within `content` that should be emphasized
    /// (word-level diff highlights).
    pub highlights: Vec<(usize, usize)>,
}

/// A preview of changed lines for an edit operation.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffPreview {
    /// Flat list of diff lines — used by the unified renderer.
    pub lines: Vec<DiffLine>,
    /// Paired rows — used by the split (side-by-side) renderer.
    pub rows: Vec<SplitDiffRow>,
    pub lang_hint: String,
    /// 1-indexed line number where the edit begins in the original file.
    pub start_line: Option<usize>,
}
