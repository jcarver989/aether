use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::coding::display_meta::{ToolDisplayMeta, truncate};
use crate::coding::error::FileError;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditFileArgs {
    /// Path to the file to edit
    pub file_path: String,
    /// Exact string to find and replace in the file
    pub old_string: String,
    /// String to replace it with
    pub new_string: String,
    /// Replace all occurrences (default: false - replace only first match)
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditFileResponse {
    pub status: String,
    /// Path of the file that was edited
    pub file_path: String,
    /// Total number of lines in the file after editing
    pub total_lines: usize,
    /// Number of replacements made
    pub replacements_made: usize,
    /// The new file content after editing (used internally for LSP sync)
    #[serde(skip_serializing)]
    pub content: String,
    /// Display metadata for human-friendly rendering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

pub async fn edit_file_contents(args: EditFileArgs) -> Result<EditFileResponse, FileError> {
    // File must exist for editing
    if !Path::new(&args.file_path).exists() {
        return Err(FileError::NotFound {
            path: args.file_path,
        });
    }

    // Read current file content
    let current_content = match std::fs::read_to_string(&args.file_path) {
        Ok(content) => content,
        Err(e) => {
            return Err(FileError::ReadFailed {
                path: args.file_path,
                reason: e.to_string(),
            });
        }
    };

    // Perform string replacement
    let (updated_content, replacements_made) = if args.replace_all {
        let count = current_content.matches(&args.old_string).count();
        (
            current_content.replace(&args.old_string, &args.new_string),
            count,
        )
    } else if current_content.contains(&args.old_string) {
        (
            current_content.replacen(&args.old_string, &args.new_string, 1),
            1,
        )
    } else {
        (current_content.clone(), 0)
    };

    // Check if any replacement actually occurred
    if replacements_made == 0 {
        return Err(FileError::PatternNotFound {
            path: args.file_path,
            pattern: args.old_string,
        });
    }

    // Write back to file
    if let Err(e) = std::fs::write(&args.file_path, &updated_content) {
        return Err(FileError::WriteFailed {
            path: args.file_path,
            reason: e.to_string(),
        });
    }

    // Count lines for response
    let total_lines = updated_content.lines().count();

    let display_meta = ToolDisplayMeta::edit_file(
        args.file_path.clone(),
        Some(truncate(&args.old_string, 100)),
        Some(truncate(&args.new_string, 100)),
    );

    Ok(EditFileResponse {
        status: "success".to_string(),
        file_path: args.file_path,
        total_lines,
        replacements_made,
        content: updated_content,
        _meta: display_meta.into_meta(),
    })
}
