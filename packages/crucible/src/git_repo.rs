use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a git repository used for evaluation purposes
pub struct GitRepo {
    path: PathBuf,
}

impl GitRepo {
    /// Clone a repository to the specified destination
    pub fn clone(url: &str, dest: &Path) -> Result<Self, GitRepoError> {
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
        }
    }
}

impl std::error::Error for GitRepoError {}
