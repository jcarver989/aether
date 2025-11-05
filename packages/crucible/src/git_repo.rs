use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a git repository used for evaluation purposes
pub struct GitRepo {
    path: PathBuf,
}

impl GitRepo {
    /// Create a GitRepo instance from an existing repository path
    pub fn from_path(path: &Path) -> Self {
        GitRepo {
            path: path.to_path_buf(),
        }
    }

    /// Clone a repository to the specified destination
    pub fn clone(url: &str, dest: &Path) -> Result<Self, GitRepoError> {
        tracing::info!("Cloning repository from {}", url);
        let output = Command::new("git")
            .arg("clone")
            .arg(url)
            .arg(dest)
            .output()
            .map_err(|e| {
                GitRepoError::CommandFailed(format!("Failed to execute git clone: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitRepoError::CloneFailed(stderr.to_string()));
        }

        Ok(GitRepo {
            path: dest.to_path_buf(),
        })
    }

    /// Checkout a specific commit, branch, or tag
    pub fn checkout(&self, reference: &str) -> Result<(), GitRepoError> {
        tracing::info!("Checking out ref: {}", reference);
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .arg("checkout")
            .arg(reference)
            .output()
            .map_err(|e| {
                GitRepoError::CommandFailed(format!("Failed to execute git checkout: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitRepoError::CheckoutFailed {
                reference: reference.to_string(),
                reason: stderr.to_string(),
            });
        }

        Ok(())
    }

    /// Get the diff between two commits
    ///
    /// Returns the output of `git diff from_commit..to_commit`
    pub fn diff(&self, from_commit: &str, to_commit: &str) -> Result<String, GitRepoError> {
        tracing::info!("Getting diff from {} to {}", from_commit, to_commit);
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .arg("diff")
            .arg(format!("{}..{}", from_commit, to_commit))
            .output()
            .map_err(|e| {
                GitRepoError::CommandFailed(format!("Failed to execute git diff: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitRepoError::DiffFailed {
                from: from_commit.to_string(),
                to: to_commit.to_string(),
                reason: stderr.to_string(),
            });
        }

        let diff = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(diff)
    }

    /// Get the path to the repository
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug)]
pub enum GitRepoError {
    CommandFailed(String),
    CloneFailed(String),
    CheckoutFailed { reference: String, reason: String },
    DiffFailed { from: String, to: String, reason: String },
}

impl std::fmt::Display for GitRepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitRepoError::CommandFailed(msg) => write!(f, "Git command failed: {}", msg),
            GitRepoError::CloneFailed(reason) => {
                write!(f, "Failed to clone repository: {}", reason)
            }
            GitRepoError::CheckoutFailed { reference, reason } => {
                write!(f, "Failed to checkout '{}': {}", reference, reason)
            }
            GitRepoError::DiffFailed { from, to, reason } => {
                write!(f, "Failed to diff '{}..{}': {}", from, to, reason)
            }
        }
    }
}

impl std::error::Error for GitRepoError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_git_diff_between_commits() {
        // Create a temporary directory for the test
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a git repo
        Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create initial file and commit
        fs::write(repo_path.join("test.txt"), "initial content\n").unwrap();
        Command::new("git")
            .args(["add", "test.txt"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Get the first commit hash
        let first_commit = String::from_utf8(
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(repo_path)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();

        // Make changes and create second commit
        fs::write(repo_path.join("test.txt"), "modified content\n").unwrap();
        fs::write(repo_path.join("new.txt"), "new file\n").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Second commit"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Get the second commit hash
        let second_commit = String::from_utf8(
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(repo_path)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();

        // Test the diff function
        let git_repo = GitRepo::from_path(repo_path);
        let diff = git_repo.diff(&first_commit, &second_commit).unwrap();

        // Verify the diff contains expected changes
        assert!(diff.contains("test.txt"), "Diff should mention test.txt");
        assert!(diff.contains("new.txt"), "Diff should mention new.txt");
        assert!(
            diff.contains("modified content") || diff.contains("+modified content"),
            "Diff should show modified content"
        );
    }
}
