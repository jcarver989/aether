use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::{create_dir_all, write};

use crate::coding::error::FileError;
use mcp_utils::display_meta::{
    DiffLine, DiffPreview, DiffTag, ToolDisplayMeta, ToolResultMeta, basename, extension_hint,
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileArgs {
    /// The absolute path to the file to write
    #[serde(alias = "file_path")]
    pub file_path: String,
    /// The content to write to the file
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileResponse {
    /// Success message
    pub message: String,
    /// Number of bytes written
    pub bytes_written: usize,
    /// File path that was written
    pub file_path: String,
    /// Display metadata for human-friendly rendering
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub meta: Option<ToolResultMeta>,
}

pub async fn write_file_contents(args: WriteFileArgs) -> Result<WriteFileResponse, FileError> {
    const MAX_DIFF_LINES: usize = 50;
    let file_path = Path::new(&args.file_path);

    if let Some(parent) = file_path.parent()
        && let Err(e) = create_dir_all(parent).await
    {
        return Err(FileError::CreateDirFailed {
            path: args.file_path,
            reason: e.to_string(),
        });
    }

    if let Err(e) = write(&args.file_path, &args.content).await {
        return Err(FileError::WriteFailed {
            path: args.file_path,
            reason: e.to_string(),
        });
    }

    let bytes_written = args.content.len();
    let display_meta = ToolDisplayMeta::new("Write file", basename(&args.file_path));

    let all_lines: Vec<_> = args.content.lines().collect();
    let is_truncated = all_lines.len() > MAX_DIFF_LINES;

    let mut lines: Vec<DiffLine> = all_lines
        .iter()
        .take(MAX_DIFF_LINES)
        .map(|&line| DiffLine {
            tag: DiffTag::Added,
            content: line.to_string(),
        })
        .collect();

    if is_truncated {
        lines.push(DiffLine {
            tag: DiffTag::Context,
            content: format!("... ({} more lines)", all_lines.len() - MAX_DIFF_LINES),
        });
    }

    let diff_preview = DiffPreview {
        lines,
        lang_hint: extension_hint(&args.file_path),
        start_line: Some(1), // Write always starts at line 1
    };

    Ok(WriteFileResponse {
        message: format!(
            "Successfully wrote {} bytes to {}",
            bytes_written, args.file_path
        ),
        bytes_written,
        file_path: args.file_path,
        meta: Some(ToolResultMeta::with_diff_preview(
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
    async fn write_file_creates_diff_preview_with_all_lines_added() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.rs");

        let content = "fn main() {\n    println!(\"Hello\");\n}\n";
        let result = write_file_contents(WriteFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            content: content.to_string(),
        })
        .await
        .unwrap();

        // Verify file was written
        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), content);

        // Verify diff preview
        let meta = result.meta.unwrap();
        let diff = meta.diff_preview.unwrap();

        // All lines should be marked as Added
        assert_eq!(diff.lines.len(), 3);
        assert!(diff.lines.iter().all(|l| l.tag == DiffTag::Added));
        assert_eq!(diff.lines[0].content, "fn main() {");
        assert_eq!(diff.lines[1].content, "    println!(\"Hello\");");
        assert_eq!(diff.lines[2].content, "}");
    }

    #[tokio::test]
    async fn write_file_diff_preview_has_correct_lang_hint() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        let result = write_file_contents(WriteFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            content: "let x = 1;".to_string(),
        })
        .await
        .unwrap();

        let diff = result.meta.unwrap().diff_preview.unwrap();
        assert_eq!(diff.lang_hint, "rs");
    }

    #[tokio::test]
    async fn write_file_diff_preview_start_line_is_one() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let result = write_file_contents(WriteFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            content: "line1\nline2\n".to_string(),
        })
        .await
        .unwrap();

        let diff = result.meta.unwrap().diff_preview.unwrap();
        assert_eq!(diff.start_line, Some(1));
    }

    #[tokio::test]
    async fn write_file_diff_preview_handles_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.txt");

        let result = write_file_contents(WriteFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            content: "".to_string(),
        })
        .await
        .unwrap();

        let diff = result.meta.unwrap().diff_preview.unwrap();
        assert!(diff.lines.is_empty());
    }

    #[tokio::test]
    async fn write_file_overwrites_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("existing.txt");
        fs::write(&file_path, "old content").unwrap();

        let new_content = "new content\nsecond line\n";
        let result = write_file_contents(WriteFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            content: new_content.to_string(),
        })
        .await
        .unwrap();

        // Verify file was overwritten
        assert_eq!(fs::read_to_string(&file_path).unwrap(), new_content);

        // Diff should show new content as added (not old as removed - this is write, not edit)
        let diff = result.meta.unwrap().diff_preview.unwrap();
        assert_eq!(diff.lines.len(), 2);
        assert!(diff.lines.iter().all(|l| l.tag == DiffTag::Added));
    }

    #[tokio::test]
    async fn write_file_truncates_large_diff_preview() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");

        // Create content with more than MAX_DIFF_LINES
        let lines: Vec<&str> = (1..=100).map(|_| "line of content").collect();
        let content = lines.join("\n");

        let result = write_file_contents(WriteFileArgs {
            file_path: file_path.to_string_lossy().to_string(),
            content: content.clone(),
        })
        .await
        .unwrap();

        // Verify full content was written to disk
        assert_eq!(fs::read_to_string(&file_path).unwrap(), content);

        // Verify diff preview is truncated
        let diff = result.meta.unwrap().diff_preview.unwrap();
        assert_eq!(diff.lines.len(), 51); // 50 content lines + 1 truncation indicator

        // Last line should be truncation indicator
        let last_line = &diff.lines[50];
        assert_eq!(last_line.tag, DiffTag::Context);
        assert!(last_line.content.starts_with("... ("));
        assert!(last_line.content.contains("50 more lines"));
    }

    #[test]
    fn write_file_args_accepts_snake_case_file_path() {
        let args: WriteFileArgs = serde_json::from_value(serde_json::json!({
            "file_path": "/tmp/out.txt",
            "content": "hello"
        }))
        .unwrap();

        assert_eq!(args.file_path, "/tmp/out.txt");
        assert_eq!(args.content, "hello");
    }
}
