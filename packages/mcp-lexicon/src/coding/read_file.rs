use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// Path to the file to read (must be an existing file)
    pub file_path: String,
    /// Starting line number to read from (1-indexed). If not specified, starts from line 1.
    #[serde(default = "default_offset")]
    pub offset: usize,
    /// Maximum number of lines to read. If not specified, reads entire file from offset.
    pub limit: Option<usize>,
}

fn default_offset() -> usize {
    1
}

pub async fn read_file_contents(args: ReadFileArgs) -> Result<serde_json::Value, String> {
    let file_path = Path::new(&args.file_path);

    if !file_path.exists() {
        return Err(format!("File does not exist: {}", args.file_path));
    }

    if !file_path.is_file() {
        return Err(format!("Path is not a file: {}", args.file_path));
    }

    match fs::read_to_string(file_path).await {
        Ok(content) => {
            let all_lines: Vec<&str> = content.lines().collect();
            let total_lines = all_lines.len();

            if args.offset == 0 {
                return Err(format!("Invalid offset for file {}: offset must be 1-indexed (start from 1)", args.file_path));
            }

            let start_idx = (args.offset - 1).min(total_lines);

            if start_idx >= total_lines {
                return Err(format!("Invalid offset for file {}: offset {} is beyond file length {}", args.file_path, args.offset, total_lines));
            }

            let selected_lines: Vec<&str> = match args.limit {
                Some(limit) => {
                    let end_idx = (start_idx + limit).min(total_lines);
                    all_lines[start_idx..end_idx].to_vec()
                },
                None => all_lines[start_idx..].to_vec(),
            };

            let lines_with_numbers: Vec<String> = selected_lines
                .iter()
                .enumerate()
                .map(|(i, line)| format!("{:5}\t{}", args.offset + i, line))
                .collect();

            let formatted_content = lines_with_numbers.join("\n");

            Ok(serde_json::json!({
                "status": "success",
                "file_path": args.file_path,
                "content": formatted_content,
                "total_lines": total_lines,
                "lines_shown": selected_lines.len(),
                "offset": args.offset,
                "limit": args.limit,
                "size": content.len()
            }))
        },
        Err(e) => Err(format!("Failed to read file {}: {}", args.file_path, e)),
    }
}
