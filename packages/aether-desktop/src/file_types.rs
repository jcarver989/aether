//! Shared file-related types.

use std::path::PathBuf;

/// A file match result from fuzzy search.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileMatch {
    /// Relative path from the working directory (for display)
    pub path: String,
    /// Absolute path for reading content
    pub absolute_path: PathBuf,
    /// File size in bytes
    pub size: u64,
}
