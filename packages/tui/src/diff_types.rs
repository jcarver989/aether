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

/// A preview of changed lines for an edit operation.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffPreview {
    pub lines: Vec<DiffLine>,
    pub lang_hint: String,
    /// 1-indexed line number where the edit begins in the original file.
    pub start_line: Option<usize>,
}
