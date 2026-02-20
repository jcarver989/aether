//! Common types and utilities shared across LSP tools

use lsp_types::{Location, Uri};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::error::LspError;

/// A location in source code (file path with range)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LocationResult {
    /// The file path
    pub file_path: String,
    /// Start line (1-indexed)
    pub start_line: u32,
    /// Start column (1-indexed)
    pub start_column: u32,
    /// End line (1-indexed)
    pub end_line: u32,
    /// End column (1-indexed)
    pub end_column: u32,
}

impl LocationResult {
    /// Create from an LSP Location
    pub fn from_location(loc: &Location) -> Self {
        let file_path = uri_to_path(&loc.uri);
        Self {
            file_path,
            // Convert from 0-indexed to 1-indexed
            start_line: loc.range.start.line + 1,
            start_column: loc.range.start.character + 1,
            end_line: loc.range.end.line + 1,
            end_column: loc.range.end.character + 1,
        }
    }
}

/// Parse a line number string to u32
pub fn parse_line(s: &str) -> Result<u32, String> {
    s.trim()
        .parse()
        .map_err(|_| format!("Invalid line number: {s}"))
}

/// Convert an LSP URI to a file path string
pub fn uri_to_path(uri: &Uri) -> String {
    let uri_str = uri.as_str();
    // Strip file:// prefix and decode
    if let Some(path) = uri_str.strip_prefix("file://") {
        // Handle Windows paths (file:///C:/...)
        if path.starts_with('/') && path.len() > 2 && path.chars().nth(2) == Some(':') {
            path[1..].to_string()
        } else {
            path.to_string()
        }
    } else {
        uri_str.to_string()
    }
}

/// Convert a file path to an LSP URI
pub fn path_to_uri(path: &Path) -> Result<Uri, LspError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };

    let uri_str = format!("file://{}", absolute.display());
    uri_str
        .parse()
        .map_err(|_| LspError::Transport(format!("Invalid path: {}", path.display())))
}
