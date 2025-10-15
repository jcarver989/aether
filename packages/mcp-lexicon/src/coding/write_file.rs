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
    /// Replace exact string occurrences in file content (similar to Claude Code's Edit tool)
    Replace {
        old_string: String,
        new_string: String,
        /// Replace all occurrences (default: false - replace only first match)
        #[serde(default)]
        replace_all: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileProps {
    /// Path where the file should be written (parent directories will be created if needed)
    pub file_path: String,
    /// Operation to perform on the file.
    /// Line numbers are 1-indexed to match read_file output format "     1	line content"
    pub operation: WriteOperation,
    /// Optional: return a preview of the file content with line numbers after writing
    /// If specified, returns up to this many lines starting from line 1
    #[serde(default)]
    pub preview_lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WriteStatus {
    Success,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileResponse {
    /// Path of the file that was written
    pub file_path: String,
    /// Total number of lines in the file after writing
    pub total_lines: usize,
}

pub async fn write_file_contents(args: WriteFileProps) -> Result<WriteFileResponse, String> {
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

    match &args.operation {
        WriteOperation::Overwrite { content } => {
            lines = content.lines().map(|s| s.to_string()).collect();
            final_has_trailing_newline = content.ends_with('\n');
        }
        WriteOperation::Replace {
            old_string,
            new_string,
            replace_all,
        } => {
            // Convert lines back to string for string replacement
            let current_content = lines.join("\n");
            let updated_content = if *replace_all {
                current_content.replace(old_string, new_string)
            } else {
                current_content.replacen(old_string, new_string, 1)
            };

            // Check if any replacement actually occurred
            if current_content == updated_content {
                return Err(format!(
                    "String replacement failed for file {}: string '{}' not found",
                    args.file_path, old_string
                ));
            }

            // Convert back to lines
            lines = updated_content.lines().map(|s| s.to_string()).collect();
            final_has_trailing_newline = updated_content.ends_with('\n');
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
                } else {
                    // Insert between existing lines
                    let insert_idx = end_idx + 1;
                    for (i, line) in new_lines.into_iter().enumerate() {
                        lines.insert(insert_idx + i, line);
                    }
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

    let response = WriteFileResponse {
        file_path: args.file_path,
        total_lines: lines.len(),
    };

    Ok(response)
}
