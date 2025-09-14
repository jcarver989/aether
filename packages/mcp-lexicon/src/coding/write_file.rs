use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WriteOperation {
    /// Replace entire file content
    Overwrite { content: String },
    /// Replace a range of lines (1-indexed, inclusive).
    /// Examples: start_line=2, end_line=4 replaces lines 2-4
    /// Use start_line = end_line + 1 to insert between lines (e.g., start_line=3, end_line=2 inserts at line 3)
    /// Use start_line > file_length to append to end of file
    LineRange {
        start_line: usize,
        end_line: usize,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    /// Path where the file should be written (parent directories will be created if needed)
    pub file_path: String,
    /// Operations to perform on the file (executed in order).
    /// Line numbers are 1-indexed to match read_file output format "   1│ line content"
    pub operations: Vec<WriteOperation>,
}

pub async fn write_file_contents(args: WriteFileArgs) -> Result<serde_json::Value, String> {
    let file_path = Path::new(&args.file_path);

    // Read current file as lines (empty vec if file doesn't exist)
    let (mut lines, original_had_trailing_newline) = if file_path.exists() {
        match fs::read_to_string(file_path).await {
            Ok(content) => {
                let had_trailing_newline = content.ends_with('\n');
                let lines = content.lines().map(|s| s.to_string()).collect::<Vec<_>>();
                (lines, had_trailing_newline)
            }
            Err(e) => {
                return Err(format!(
                    "Failed to read existing file {}: {}",
                    args.file_path, e
                ));
            }
        }
    } else {
        (Vec::new(), false)
    };

    let mut final_has_trailing_newline = original_had_trailing_newline;

    let mut applied_operations = Vec::new();

    // Apply each operation sequentially
    for operation in &args.operations {
        match operation {
            WriteOperation::Overwrite { content } => {
                lines = content.lines().map(|s| s.to_string()).collect();
                final_has_trailing_newline = content.ends_with('\n');
                applied_operations.push("Overwrite entire file".to_string());
            }
            WriteOperation::LineRange {
                start_line,
                end_line,
                content,
            } => {
                // Convert to 0-indexed
                let start_idx = start_line.saturating_sub(1);
                let end_idx = end_line.saturating_sub(1);

                // Handle insertion (start = end + 1) or append when start_line is beyond file length
                if start_idx > end_idx || start_idx >= lines.len() {
                    let new_lines: Vec<_> = content.lines().map(|s| s.to_string()).collect();

                    // If start_line is beyond file length, append to end
                    if start_idx >= lines.len() {
                        lines.extend(new_lines);
                        final_has_trailing_newline = content.ends_with('\n');
                        applied_operations
                            .push(format!("Appended {} lines at end", content.lines().count()));
                    } else {
                        // Insert between existing lines
                        let insert_idx = end_idx + 1;
                        for (i, line) in new_lines.into_iter().enumerate() {
                            lines.insert(insert_idx + i, line);
                        }
                        applied_operations.push(format!(
                            "Inserted {} lines at line {}",
                            content.lines().count(),
                            start_line
                        ));
                    }
                } else {
                    // Replace range
                    let new_lines: Vec<_> = content.lines().map(|s| s.to_string()).collect();

                    // Extend lines if necessary to accommodate the range
                    while lines.len() <= end_idx {
                        lines.push(String::new());
                    }

                    // Replace the range
                    lines.splice(start_idx..=end_idx, new_lines);
                    applied_operations.push(format!("Replaced lines {}-{}", start_line, end_line));
                }
            }
        }
    }

    // Create parent directories if needed
    if let Some(parent) = file_path.parent() {
        if let Err(e) = fs::create_dir_all(parent).await {
            return Err(format!(
                "Failed to create directories for {}: {}",
                args.file_path, e
            ));
        }
    }

    // Join lines and write, preserving trailing newline if needed
    let final_content = if final_has_trailing_newline && !lines.is_empty() {
        format!("{}\n", lines.join("\n"))
    } else {
        lines.join("\n")
    };
    if let Err(e) = fs::write(file_path, final_content).await {
        return Err(format!("Failed to write to file {}: {}", args.file_path, e));
    }

    Ok(serde_json::json!({
        "status": "success",
        "file_path": args.file_path,
        "operations_applied": applied_operations,
        "total_lines": lines.len()
    }))
}
