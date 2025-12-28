//! Git diff computation engine using git2-rs.
//!
//! Provides functionality to compute diffs between HEAD and the working directory.

use crate::error::AetherDesktopError;
use crate::state::{DiffHunk, DiffLine, FileDiff, FileStatus, LineOrigin};
use git2::{Delta, DiffOptions, Patch, Repository};
use std::fs::read_to_string;
use std::path::Path;

/// Compute the diff between HEAD and the working directory.
///
/// Returns a list of file diffs showing what has changed.
/// If the path is not a git repository, returns `AetherDesktopError::DiffNotRepository`.
pub fn compute_diff(repo_path: &Path) -> Result<Vec<FileDiff>, AetherDesktopError> {
    let repo = Repository::discover(repo_path)?;
    let workdir = repo.workdir().unwrap_or(repo_path);

    // Get HEAD tree (handle case where repo has no commits)
    let head_tree = match repo.head() {
        Ok(head) => Some(head.peel_to_tree()?),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    // Configure diff options
    let mut opts = DiffOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);

    // Compute diff: HEAD tree vs working directory
    let diff = repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))?;

    let mut file_diffs = Vec::new();
    let num_deltas = diff.deltas().len();

    for idx in 0..num_deltas {
        let delta = diff.get_delta(idx).ok_or_else(|| {
            AetherDesktopError::DiffGit(git2::Error::from_str("Delta index out of bounds"))
        })?;

        let status = match delta.status() {
            Delta::Added | Delta::Untracked => FileStatus::Added,
            Delta::Deleted => FileStatus::Deleted,
            Delta::Modified => FileStatus::Modified,
            Delta::Renamed => FileStatus::Renamed,
            // Treat copied files and type changes as modifications
            Delta::Copied | Delta::Typechange => FileStatus::Modified,
            // Ignore conflicted, ignored, and unreadable entries
            Delta::Conflicted | Delta::Ignored | Delta::Unmodified | Delta::Unreadable => continue,
        };

        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .ok_or(AetherDesktopError::DiffMissingPath)?;

        let old_path = if delta.status() == Delta::Renamed {
            delta
                .old_file()
                .path()
                .map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };

        // Get the patch for this file to extract hunks
        let mut hunks = match Patch::from_diff(&diff, idx)? {
            Some(patch) => parse_patch(&patch)?,
            None => Vec::new(),
        };

        // For new/untracked files, git2 may not produce a proper patch.
        // In that case, read the file and create synthetic hunks.
        if hunks.is_empty() && status == FileStatus::Added {
            hunks = create_added_file_hunks(workdir, &path);
        }

        file_diffs.push(FileDiff {
            path,
            old_path,
            status,
            hunks,
        });
    }

    Ok(file_diffs)
}

/// Create synthetic hunks for a newly added file by reading its contents.
fn create_added_file_hunks(workdir: &Path, file_path: &str) -> Vec<DiffHunk> {
    let full_path = workdir.join(file_path);

    // Try to read the file contents
    let content = match read_to_string(&full_path) {
        Ok(content) => content,
        Err(_) => return Vec::new(), // Binary file or read error
    };

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

    vec![DiffHunk {
        old_start: 0,
        old_lines: 0,
        new_start: 1,
        new_lines: num_lines,
        lines,
    }]
}

/// Parse hunks from a git2 Patch.
fn parse_patch(patch: &Patch<'_>) -> Result<Vec<DiffHunk>, AetherDesktopError> {
    let mut hunks = Vec::new();

    for hunk_idx in 0..patch.num_hunks() {
        let (hunk, num_lines) = patch.hunk(hunk_idx)?;

        let mut lines = Vec::new();
        for line_idx in 0..num_lines {
            let line = patch.line_in_hunk(hunk_idx, line_idx)?;

            let origin = match line.origin() {
                '+' => LineOrigin::Addition,
                '-' => LineOrigin::Deletion,
                ' ' => LineOrigin::Context,
                _ => continue, // Skip header lines, etc.
            };

            lines.push(DiffLine {
                origin,
                old_lineno: line.old_lineno(),
                new_lineno: line.new_lineno(),
                content: String::from_utf8_lossy(line.content()).to_string(),
            });
        }

        hunks.push(DiffHunk {
            old_start: hunk.old_start(),
            old_lines: hunk.old_lines(),
            new_start: hunk.new_start(),
            new_lines: hunk.new_lines(),
            lines,
        });
    }

    Ok(hunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_git_repo(dir: &Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .expect("Failed to init git repo");

        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()
            .expect("Failed to set git email");

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .expect("Failed to set git name");
    }

    fn git_add_and_commit(dir: &Path, message: &str) {
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir)
            .output()
            .expect("Failed to git add");

        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(dir)
            .output()
            .expect("Failed to git commit");
    }

    #[test]
    fn test_compute_diff_non_repo() {
        let temp_dir = TempDir::new().unwrap();
        let result = compute_diff(temp_dir.path());
        assert!(matches!(result, Err(AetherDesktopError::DiffNotRepository)));
    }

    #[test]
    fn test_compute_diff_empty_repo() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        let result = compute_diff(temp_dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_compute_diff_added_file() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        // Create initial commit
        fs::write(temp_dir.path().join("initial.txt"), "initial").unwrap();
        git_add_and_commit(temp_dir.path(), "Initial commit");

        // Add a new file
        fs::write(temp_dir.path().join("new_file.txt"), "new content").unwrap();

        let result = compute_diff(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "new_file.txt");
        assert_eq!(result[0].status, FileStatus::Added);
    }

    #[test]
    fn test_compute_diff_added_file_has_content() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        // Create initial commit
        fs::write(temp_dir.path().join("initial.txt"), "initial").unwrap();
        git_add_and_commit(temp_dir.path(), "Initial commit");

        // Add a new file with multiple lines
        fs::write(
            temp_dir.path().join("new_file.txt"),
            "line 1\nline 2\nline 3\n",
        )
        .unwrap();

        let result = compute_diff(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "new_file.txt");
        assert_eq!(result[0].status, FileStatus::Added);

        // The key assertion: new files should have hunks showing the content
        assert!(
            !result[0].hunks.is_empty(),
            "New files should have hunks showing the content"
        );

        // All lines should be additions
        let all_additions = result[0]
            .hunks
            .iter()
            .flat_map(|h| &h.lines)
            .all(|line| line.origin == LineOrigin::Addition);
        assert!(all_additions, "All lines in a new file should be additions");
    }

    #[test]
    fn test_compute_diff_modified_file() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        // Create initial commit
        fs::write(temp_dir.path().join("file.txt"), "original content").unwrap();
        git_add_and_commit(temp_dir.path(), "Initial commit");

        // Modify the file
        fs::write(temp_dir.path().join("file.txt"), "modified content").unwrap();

        let result = compute_diff(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "file.txt");
        assert_eq!(result[0].status, FileStatus::Modified);
        assert!(!result[0].hunks.is_empty());
    }

    #[test]
    fn test_compute_diff_deleted_file() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        // Create initial commit
        fs::write(temp_dir.path().join("file.txt"), "content").unwrap();
        git_add_and_commit(temp_dir.path(), "Initial commit");

        // Delete the file
        fs::remove_file(temp_dir.path().join("file.txt")).unwrap();

        let result = compute_diff(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "file.txt");
        assert_eq!(result[0].status, FileStatus::Deleted);
    }

    #[test]
    fn test_diff_state_default() {
        use crate::state::DiffState;
        let state = DiffState::default();
        assert!(state.files.is_empty());
        assert!(state.selected_file.is_none());
        assert!(!state.loading);
        assert!(state.error.is_none());
    }
}
