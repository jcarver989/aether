use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::fs::write;

use similar::{ChangeTag, TextDiff};

use crate::coding::error::FileError;
use crate::coding::tools::file_io::read_text_file;
use mcp_utils::display_meta::{
    DiffLine, DiffPreview, DiffTag, ToolDisplayMeta, ToolResultMeta, basename, extension_hint,
};

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
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub _meta: Option<ToolResultMeta>,
}

pub async fn edit_file_contents(args: EditFileArgs) -> Result<EditFileResponse, FileError> {
    // Read current file content
    let current_content = read_text_file(&args.file_path).await?;

    // Perform string replacement, capturing the first match offset for diff preview
    let (updated_content, replacements_made, first_match_offset) = if args.replace_all {
        let first_offset = current_content.find(&args.old_string);
        let count = current_content.matches(&args.old_string).count();
        (
            current_content.replace(&args.old_string, &args.new_string),
            count,
            first_offset,
        )
    } else if let Some(offset) = current_content.find(&args.old_string) {
        (
            current_content.replacen(&args.old_string, &args.new_string, 1),
            1,
            Some(offset),
        )
    } else {
        (current_content.clone(), 0, None)
    };

    // Check if any replacement actually occurred
    if replacements_made == 0 {
        return Err(FileError::PatternNotFound {
            path: args.file_path,
            pattern: args.old_string,
        });
    }

    // Write back to file
    if let Err(e) = write(&args.file_path, &updated_content).await {
        return Err(FileError::WriteFailed {
            path: args.file_path,
            reason: e.to_string(),
        });
    }

    // Count lines for response
    let total_lines = updated_content.lines().count();

    let start_line =
        first_match_offset.map(|byte_offset| current_content[..byte_offset].lines().count() + 1);

    let display_meta = ToolDisplayMeta::new("Edit file", basename(&args.file_path));
    let diff = TextDiff::from_lines(&args.old_string, &args.new_string);
    let lines: Vec<DiffLine> = diff
        .iter_all_changes()
        .map(|change| {
            let tag = match change.tag() {
                ChangeTag::Equal => DiffTag::Context,
                ChangeTag::Delete => DiffTag::Removed,
                ChangeTag::Insert => DiffTag::Added,
            };
            DiffLine {
                tag,
                content: change.value().trim_end_matches('\n').to_string(),
            }
        })
        .collect();
    let diff_preview = DiffPreview {
        lines,
        lang_hint: extension_hint(&args.file_path),
        start_line,
    };

    Ok(EditFileResponse {
        status: "success".to_string(),
        file_path: args.file_path,
        total_lines,
        replacements_made,
        content: updated_content,
        _meta: Some(ToolResultMeta::with_diff_preview(
            display_meta,
            diff_preview,
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_utils::display_meta::DiffTag;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn edit_file_nonexistent_returns_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("missing.txt");

        let result = edit_file_contents(EditFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "before".to_string(),
            new_string: "after".to_string(),
            replace_all: false,
        })
        .await;

        assert!(matches!(result, Err(FileError::NotFound { .. })));
    }

    #[tokio::test]
    async fn edit_file_existing_file_without_match_returns_pattern_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("sample.txt");
        fs::write(&file_path, "hello world").unwrap();

        let result = edit_file_contents(EditFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "missing".to_string(),
            new_string: "replacement".to_string(),
            replace_all: false,
        })
        .await;

        assert!(matches!(result, Err(FileError::PatternNotFound { .. })));
    }

    #[tokio::test]
    async fn edit_file_sets_start_line_in_diff_preview() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("lines.txt");
        fs::write(&file_path, "line1\nline2\nline3\nline4\n").unwrap();

        let result = edit_file_contents(EditFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "line3".to_string(),
            new_string: "replaced".to_string(),
            replace_all: false,
        })
        .await
        .unwrap();

        let meta = result._meta.unwrap();
        let diff = meta.diff_preview.unwrap();
        assert_eq!(diff.start_line, Some(3));
    }

    #[tokio::test]
    async fn edit_file_start_line_is_one_for_first_line() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("first.txt");
        fs::write(&file_path, "hello world\nsecond line\n").unwrap();

        let result = edit_file_contents(EditFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "hello".to_string(),
            new_string: "goodbye".to_string(),
            replace_all: false,
        })
        .await
        .unwrap();

        let meta = result._meta.unwrap();
        let diff = meta.diff_preview.unwrap();
        assert_eq!(diff.start_line, Some(1));
    }

    #[tokio::test]
    async fn edit_file_diff_preview_only_marks_changed_lines() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("multi.rs");
        let content = "fn main() {\n    let x = 1;\n    let y = 2;\n    let z = 3;\n}\n";
        fs::write(&file_path, content).unwrap();

        let old_string = "    let x = 1;\n    let y = 2;\n    let z = 3;\n";
        let new_string = "    let x = 1;\n    let y = 99;\n    let z = 3;\n";

        let result = edit_file_contents(EditFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: old_string.to_string(),
            new_string: new_string.to_string(),
            replace_all: false,
        })
        .await
        .unwrap();

        let diff = result._meta.unwrap().diff_preview.unwrap();
        let tags: Vec<DiffTag> = diff.lines.iter().map(|l| l.tag).collect();
        assert_eq!(
            tags,
            vec![
                DiffTag::Context, // let x = 1;
                DiffTag::Removed, // let y = 2;
                DiffTag::Added,   // let y = 99;
                DiffTag::Context, // let z = 3;
            ],
        );
    }

    #[tokio::test]
    async fn edit_file_diff_preview_all_lines_differ() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("all_diff.txt");
        fs::write(&file_path, "aaa\nbbb\n").unwrap();

        let result = edit_file_contents(EditFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "aaa\nbbb\n".to_string(),
            new_string: "xxx\nyyy\n".to_string(),
            replace_all: false,
        })
        .await
        .unwrap();

        let diff = result._meta.unwrap().diff_preview.unwrap();
        for line in &diff.lines {
            assert_ne!(line.tag, DiffTag::Context, "no context lines expected");
        }
    }

    #[tokio::test]
    async fn edit_file_diff_preview_pure_insertion() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("insert.txt");
        fs::write(&file_path, "before\nafter\n").unwrap();

        let result = edit_file_contents(EditFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "before\nafter".to_string(),
            new_string: "before\nnew line\nafter".to_string(),
            replace_all: false,
        })
        .await
        .unwrap();

        let diff = result._meta.unwrap().diff_preview.unwrap();
        let tags: Vec<DiffTag> = diff.lines.iter().map(|l| l.tag).collect();
        assert!(tags.contains(&DiffTag::Added));
        // The unchanged lines should be context
        assert!(tags.contains(&DiffTag::Context));
        // No lines should be removed
        assert!(!tags.contains(&DiffTag::Removed));
    }

    #[tokio::test]
    async fn edit_file_diff_preview_pure_deletion() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("delete.txt");
        fs::write(&file_path, "keep\nremove me\nkeep too\n").unwrap();

        let result = edit_file_contents(EditFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            old_string: "keep\nremove me\nkeep too".to_string(),
            new_string: "keep\nkeep too".to_string(),
            replace_all: false,
        })
        .await
        .unwrap();

        let diff = result._meta.unwrap().diff_preview.unwrap();
        let tags: Vec<DiffTag> = diff.lines.iter().map(|l| l.tag).collect();
        assert!(tags.contains(&DiffTag::Removed));
        assert!(tags.contains(&DiffTag::Context));
        assert!(!tags.contains(&DiffTag::Added));
    }
}
