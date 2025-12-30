//! Git diff computation for Docker containers via docker exec.
//!
//! Provides functionality to compute diffs inside containers by running
//! git commands via the agent handle's exec method.

use crate::state::{DiffHunk, DiffLine, FileDiff, FileStatus, LineOrigin};
use aether_acp_client::AgentProcess;

/// Error type for Docker diff operations.
#[derive(Debug, thiserror::Error)]
pub enum DockerDiffError {
    #[error("Exec failed: {0}")]
    ExecFailed(String),
}

/// Compute diff by running git commands inside the container.
pub async fn compute_docker_diff(
    handle: &dyn AgentProcess,
) -> Result<Vec<FileDiff>, DockerDiffError> {
    // Get list of changed files with status
    let status_output = handle
        .exec(vec![
            "git".to_string(),
            "status".to_string(),
            "--porcelain".to_string(),
        ])
        .await
        .map_err(|e| DockerDiffError::ExecFailed(e.to_string()))?;

    // Get the actual diff content
    let diff_output = handle
        .exec(vec![
            "git".to_string(),
            "diff".to_string(),
            "--no-color".to_string(),
            "HEAD".to_string(),
        ])
        .await
        .map_err(|e| DockerDiffError::ExecFailed(e.to_string()))?;

    // Also get diff for untracked files (new files)
    let untracked_files = parse_untracked_files(&status_output);

    // Parse the unified diff
    let mut files = parse_unified_diff(&diff_output)?;

    // Add untracked files by reading their content
    for path in untracked_files {
        if let Ok(content) = handle.exec(vec!["cat".to_string(), path.clone()]).await {
            let file_diff = create_added_file_diff(&path, &content);
            files.push(file_diff);
        }
    }

    Ok(files)
}

/// Parse untracked files from git status --porcelain output.
fn parse_untracked_files(status_output: &str) -> Vec<String> {
    status_output
        .lines()
        .filter_map(|line| {
            if line.starts_with("??") {
                // Untracked file format: "?? path/to/file"
                Some(line[3..].trim().to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Create a FileDiff for a newly added file from its content.
fn create_added_file_diff(path: &str, content: &str) -> FileDiff {
    let lines: Vec<DiffLine> = content
        .lines()
        .enumerate()
        .map(|(idx, line)| DiffLine {
            origin: LineOrigin::Addition,
            old_lineno: None,
            new_lineno: Some((idx + 1) as u32),
            content: format!("{}\n", line),
        })
        .collect();

    let num_lines = lines.len() as u32;

    let hunks = if lines.is_empty() {
        vec![]
    } else {
        vec![DiffHunk {
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: num_lines,
            lines,
        }]
    };

    FileDiff {
        path: path.to_string(),
        old_path: None,
        status: FileStatus::Added,
        hunks,
    }
}

/// Parse unified diff output from `git diff` into FileDiff structures.
pub fn parse_unified_diff(diff_output: &str) -> Result<Vec<FileDiff>, DockerDiffError> {
    let mut files = Vec::new();
    let mut current_file: Option<FileDiff> = None;
    let mut current_hunk: Option<DiffHunk> = None;
    let mut old_line = 0u32;
    let mut new_line = 0u32;

    for line in diff_output.lines() {
        if line.starts_with("diff --git") {
            // Save previous file if exists
            if let Some(mut file) = current_file.take() {
                if let Some(hunk) = current_hunk.take() {
                    file.hunks.push(hunk);
                }
                files.push(file);
            }

            // Parse new file path from "diff --git a/path b/path"
            let path = parse_diff_header(line);
            current_file = Some(FileDiff {
                path,
                old_path: None,
                status: FileStatus::Modified, // Will be updated by subsequent headers
                hunks: Vec::new(),
            });
        } else if line.starts_with("new file mode") {
            if let Some(ref mut file) = current_file {
                file.status = FileStatus::Added;
            }
        } else if line.starts_with("deleted file mode") {
            if let Some(ref mut file) = current_file {
                file.status = FileStatus::Deleted;
            }
        } else if let Some(stripped) = line.strip_prefix("rename from ") {
            if let Some(ref mut file) = current_file {
                file.old_path = Some(stripped.to_string());
                file.status = FileStatus::Renamed;
            }
        } else if line.starts_with("@@") {
            // Save previous hunk if exists
            if let Some(ref mut file) = current_file
                && let Some(hunk) = current_hunk.take()
            {
                file.hunks.push(hunk);
            }

            // Parse hunk header: @@ -old_start,old_lines +new_start,new_lines @@
            if let Some((os, ol, ns, nl)) = parse_hunk_header(line) {
                old_line = os;
                new_line = ns;
                current_hunk = Some(DiffHunk {
                    old_start: os,
                    old_lines: ol,
                    new_start: ns,
                    new_lines: nl,
                    lines: Vec::new(),
                });
            }
        } else if let Some(ref mut hunk) = current_hunk {
            // Parse diff lines
            let first_char = line.chars().next();
            match first_char {
                Some('+') => {
                    hunk.lines.push(DiffLine {
                        origin: LineOrigin::Addition,
                        old_lineno: None,
                        new_lineno: Some(new_line),
                        content: format!("{}\n", &line[1..]),
                    });
                    new_line += 1;
                }
                Some('-') => {
                    hunk.lines.push(DiffLine {
                        origin: LineOrigin::Deletion,
                        old_lineno: Some(old_line),
                        new_lineno: None,
                        content: format!("{}\n", &line[1..]),
                    });
                    old_line += 1;
                }
                Some(' ') => {
                    hunk.lines.push(DiffLine {
                        origin: LineOrigin::Context,
                        old_lineno: Some(old_line),
                        new_lineno: Some(new_line),
                        content: format!("{}\n", &line[1..]),
                    });
                    old_line += 1;
                    new_line += 1;
                }
                _ => {
                    // Skip other lines (binary, etc.)
                }
            }
        }
    }

    // Save last file
    if let Some(mut file) = current_file {
        if let Some(hunk) = current_hunk {
            file.hunks.push(hunk);
        }
        files.push(file);
    }

    Ok(files)
}

/// Parse file path from diff header: "diff --git a/path b/path"
fn parse_diff_header(line: &str) -> String {
    // Format: "diff --git a/path/to/file b/path/to/file"
    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() >= 4 {
        // Get the b/path part and strip the "b/" prefix
        let b_path = parts.last().unwrap_or(&"");
        b_path.strip_prefix("b/").unwrap_or(b_path).to_string()
    } else {
        String::new()
    }
}

/// Parse hunk header: @@ -old_start,old_lines +new_start,new_lines @@
fn parse_hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    // Format: "@@ -1,5 +1,7 @@" or "@@ -1 +1,2 @@" (count defaults to 1)
    let line = line.trim_start_matches("@@").trim();
    let parts: Vec<&str> = line.split("@@").next()?.split_whitespace().collect();

    if parts.len() < 2 {
        return None;
    }

    let parse_range = |s: &str| -> (u32, u32) {
        let s = s.trim_start_matches(['-', '+']);
        let parts: Vec<&str> = s.split(',').collect();
        let start = parts.first().and_then(|s| s.parse().ok()).unwrap_or(1);
        let count = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
        (start, count)
    };

    let (old_start, old_lines) = parse_range(parts[0]);
    let (new_start, new_lines) = parse_range(parts[1]);

    Some((old_start, old_lines, new_start, new_lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unified_diff_modified() {
        let diff = r#"diff --git a/file.txt b/file.txt
index 1234567..abcdefg 100644
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,4 @@
 line 1
-line 2
+line 2 modified
+line 2.5
 line 3
"#;
        let files = parse_unified_diff(diff).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "file.txt");
        assert_eq!(files[0].status, FileStatus::Modified);
        assert_eq!(files[0].hunks.len(), 1);
        assert_eq!(files[0].hunks[0].lines.len(), 5);
    }

    #[test]
    fn test_parse_unified_diff_added() {
        let diff = r#"diff --git a/new.txt b/new.txt
new file mode 100644
index 0000000..1234567
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+line 1
+line 2
"#;
        let files = parse_unified_diff(diff).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "new.txt");
        assert_eq!(files[0].status, FileStatus::Added);
    }

    #[test]
    fn test_parse_unified_diff_deleted() {
        let diff = r#"diff --git a/old.txt b/old.txt
deleted file mode 100644
index 1234567..0000000
--- a/old.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-line 1
-line 2
"#;
        let files = parse_unified_diff(diff).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "old.txt");
        assert_eq!(files[0].status, FileStatus::Deleted);
    }

    #[test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -1,5 +1,7 @@"), Some((1, 5, 1, 7)));
        assert_eq!(parse_hunk_header("@@ -1 +1,2 @@"), Some((1, 1, 1, 2)));
        assert_eq!(
            parse_hunk_header("@@ -10,20 +15,25 @@ context"),
            Some((10, 20, 15, 25))
        );
    }

    #[test]
    fn test_parse_diff_header() {
        assert_eq!(
            parse_diff_header("diff --git a/src/main.rs b/src/main.rs"),
            "src/main.rs"
        );
        assert_eq!(
            parse_diff_header("diff --git a/file.txt b/file.txt"),
            "file.txt"
        );
    }

    #[test]
    fn test_parse_untracked_files() {
        let status = "M  modified.txt\n?? new_file.txt\n?? another/path.rs\nA  added.txt";
        let untracked = parse_untracked_files(status);
        assert_eq!(untracked.len(), 2);
        assert_eq!(untracked[0], "new_file.txt");
        assert_eq!(untracked[1], "another/path.rs");
    }

    #[test]
    fn test_create_added_file_diff() {
        let content = "line 1\nline 2\nline 3";
        let diff = create_added_file_diff("test.txt", content);
        assert_eq!(diff.path, "test.txt");
        assert_eq!(diff.status, FileStatus::Added);
        assert_eq!(diff.hunks.len(), 1);
        assert_eq!(diff.hunks[0].lines.len(), 3);
        assert!(
            diff.hunks[0]
                .lines
                .iter()
                .all(|l| l.origin == LineOrigin::Addition)
        );
    }
}
