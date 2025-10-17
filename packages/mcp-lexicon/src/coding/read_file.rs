use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

const MAX_LINE_LENGTH: usize = 2000;
const DEFAULT_LINE_LIMIT: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// Path to the file to read (must be an existing file)
    pub file_path: String,
    /// Starting line number to read from (1-indexed). If not specified, starts from line 1.
    pub offset: Option<usize>,
    /// Maximum number of lines to read. If not specified, reads up to 2000 lines.
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileResult {
    pub status: String,
    pub file_path: String,
    pub content: String,
    pub total_lines: usize,
    pub lines_shown: usize,
    pub offset: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    pub size: usize,
}

pub async fn read_file_contents(args: ReadFileArgs) -> Result<ReadFileResult, String> {
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

            // Default offset to 1 if not provided
            let offset = args.offset.unwrap_or(1);

            // Validate offset is 1-indexed
            if offset == 0 {
                return Err(format!(
                    "Invalid offset for file {}: offset must be 1-indexed (start from 1)",
                    args.file_path
                ));
            }

            let start_idx = (offset - 1).min(total_lines);

            // Apply limit with default of DEFAULT_LINE_LIMIT
            let limit = args.limit.unwrap_or(DEFAULT_LINE_LIMIT);
            let end_idx = (start_idx + limit).min(total_lines);
            let selected_lines: Vec<&str> = all_lines[start_idx..end_idx].to_vec();

            // Format lines with numbers and truncate long lines
            let lines_with_numbers: Vec<String> = selected_lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let line_num = offset + i;
                    if line.len() > MAX_LINE_LENGTH {
                        format!(
                            "{:5}\t{}... [truncated, {} chars total]",
                            line_num,
                            &line[..MAX_LINE_LENGTH],
                            line.len()
                        )
                    } else {
                        format!("{:5}\t{}", line_num, line)
                    }
                })
                .collect();

            let formatted_content = lines_with_numbers.join("\n");

            Ok(ReadFileResult {
                status: "success".to_string(),
                file_path: args.file_path,
                content: formatted_content,
                total_lines,
                lines_shown: selected_lines.len(),
                offset,
                limit: Some(limit),
                size: content.len(),
            })
        }
        Err(e) => Err(format!("Failed to read file {}: {}", args.file_path, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_file_with_defaults() {
        let test_content = "line 1\nline 2\nline 3";
        let test_path = "/tmp/test_read_defaults.txt";
        fs::write(test_path, test_content).await.unwrap();

        let result = read_file_contents(ReadFileArgs {
            file_path: test_path.to_string(),
            offset: None,
            limit: None,
        })
        .await
        .unwrap();

        assert_eq!(result.status, "success");
        assert_eq!(result.total_lines, 3);
        assert_eq!(result.lines_shown, 3);
        assert_eq!(result.offset, 1);
        assert_eq!(result.limit, Some(DEFAULT_LINE_LIMIT));
        assert!(result.content.contains("    1\tline 1"));
        assert!(result.content.contains("    2\tline 2"));
        assert!(result.content.contains("    3\tline 3"));

        let _ = fs::remove_file(test_path).await;
    }

    #[tokio::test]
    async fn test_read_file_with_offset_and_limit() {
        let test_content = "line 1\nline 2\nline 3\nline 4\nline 5";
        let test_path = "/tmp/test_read_offset_limit.txt";
        fs::write(test_path, test_content).await.unwrap();

        let result = read_file_contents(ReadFileArgs {
            file_path: test_path.to_string(),
            offset: Some(2),
            limit: Some(2),
        })
        .await
        .unwrap();

        assert_eq!(result.total_lines, 5);
        assert_eq!(result.lines_shown, 2);
        assert_eq!(result.offset, 2);
        assert_eq!(result.limit, Some(2));
        assert_eq!(result.content, "    2\tline 2\n    3\tline 3");

        let _ = fs::remove_file(test_path).await;
    }

    #[tokio::test]
    async fn test_read_file_line_truncation() {
        let short_line = "short";
        let long_line = "x".repeat(2500);
        let test_content = format!("{}\n{}", short_line, long_line);
        let test_path = "/tmp/test_read_truncation.txt";
        fs::write(test_path, &test_content).await.unwrap();

        let result = read_file_contents(ReadFileArgs {
            file_path: test_path.to_string(),
            offset: None,
            limit: None,
        })
        .await
        .unwrap();

        let lines: Vec<&str> = result.content.split('\n').collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("short"));
        assert!(!lines[0].contains("truncated"));
        assert!(lines[1].contains("truncated"));
        assert!(lines[1].contains("2500 chars total"));

        let _ = fs::remove_file(test_path).await;
    }

    #[tokio::test]
    async fn test_read_file_default_limit() {
        // Create file with more than DEFAULT_LINE_LIMIT lines
        let mut lines = Vec::new();
        for i in 1..=2500 {
            lines.push(format!("Line {}", i));
        }
        let test_content = lines.join("\n");
        let test_path = "/tmp/test_read_default_limit.txt";
        fs::write(test_path, &test_content).await.unwrap();

        let result = read_file_contents(ReadFileArgs {
            file_path: test_path.to_string(),
            offset: None,
            limit: None,
        })
        .await
        .unwrap();

        assert_eq!(result.total_lines, 2500);
        assert_eq!(result.lines_shown, DEFAULT_LINE_LIMIT);
        assert_eq!(result.limit, Some(DEFAULT_LINE_LIMIT));
        assert!(result.content.contains("    1\tLine 1"));
        assert!(result.content.contains(" 2000\tLine 2000"));
        assert!(!result.content.contains("Line 2001"));

        let _ = fs::remove_file(test_path).await;
    }

    #[tokio::test]
    async fn test_read_file_invalid_offset() {
        let test_content = "line 1";
        let test_path = "/tmp/test_read_invalid_offset.txt";
        fs::write(test_path, test_content).await.unwrap();

        let result = read_file_contents(ReadFileArgs {
            file_path: test_path.to_string(),
            offset: Some(0),
            limit: None,
        })
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("offset must be 1-indexed"));

        let _ = fs::remove_file(test_path).await;
    }

    #[tokio::test]
    async fn test_read_file_nonexistent() {
        let result = read_file_contents(ReadFileArgs {
            file_path: "/tmp/nonexistent_file_xyz123.txt".to_string(),
            offset: None,
            limit: None,
        })
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }
}
