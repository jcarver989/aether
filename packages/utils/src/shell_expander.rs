use futures::future::try_join_all;
use regex::Regex;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::{env, fmt};
use tokio::process::Command;

/// Expands `` !`command` `` markers in text by running each command via
/// `$SHELL -c` (fallback `sh`) and substituting the trimmed stdout.
///
/// Construct once and reuse across a batch of expansions to amortize regex
/// compilation.
pub struct ShellExpander {
    regex: Regex,
}

impl ShellExpander {
    pub fn new() -> Self {
        Self { regex: Regex::new(r"!`([^`\n]+)`").expect("valid regex") }
    }

    /// Expand `` !`command` `` markers in `content`, running each command from
    /// `cwd`. Returns `content` unchanged if no markers are present.
    ///
    /// Markers are expanded concurrently; the first non-zero exit or spawn
    /// failure short-circuits and surfaces as [`ShellInterpError`].
    pub async fn expand(&self, content: &str, cwd: &Path) -> Result<String, ShellExpansionError> {
        if !self.regex.is_match(content) {
            return Ok(content.to_string());
        }

        let spans: Vec<(usize, usize, &str)> = self
            .regex
            .captures_iter(content)
            .flat_map(|captures| {
                let whole = captures.get(0)?;
                let cmd = captures.get(1)?;
                Some((whole.start(), whole.end(), cmd.as_str()))
            })
            .collect();

        let outputs = try_join_all(spans.iter().map(|(_, _, cmd)| Self::run(cmd, cwd))).await?;
        let mut out = String::with_capacity(content.len());
        let mut last = 0;

        for ((start, end, _), output) in spans.iter().zip(outputs.iter()) {
            out.push_str(&content[last..*start]);
            out.push_str(output);
            last = *end;
        }

        out.push_str(&content[last..]);
        Ok(out)
    }

    async fn run(cmd: &str, cwd: &Path) -> Result<String, ShellExpansionError> {
        let shell = env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
        let output = Command::new(&shell).arg("-c").arg(cmd).current_dir(cwd).output().await.map_err(|e| {
            ShellExpansionError::Spawn { shell: shell.clone(), cmd: cmd.to_string(), error: e.to_string() }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ShellExpansionError::NonZeroExit {
                cmd: cmd.to_string(),
                status: output.status.to_string(),
                stderr: stderr.trim().to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
    }
}

#[derive(Debug)]
pub enum ShellExpansionError {
    Spawn { shell: String, cmd: String, error: String },
    NonZeroExit { cmd: String, status: String, stderr: String },
}

impl Default for ShellExpander {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ShellExpansionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { shell, cmd, error } => {
                write!(f, "Failed to spawn {shell} for `{cmd}`: {error}")
            }
            Self::NonZeroExit { cmd, status, stderr } => {
                write!(f, "Shell interpolation `{cmd}` failed with {status}: {stderr}")
            }
        }
    }
}

impl std::error::Error for ShellExpansionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_op_without_marker() {
        let content = "Just some plain content with no directives";
        let expander = ShellExpander::new();
        let cwd = std::env::current_dir().unwrap();
        let result = expander.expand(content, &cwd).await.unwrap();
        assert_eq!(result, content);
    }

    #[tokio::test]
    async fn runs_shell_command() {
        let expander = ShellExpander::new();
        let cwd = std::env::current_dir().unwrap();
        let result = expander.expand("branch: !`echo main`", &cwd).await.unwrap();
        assert_eq!(result, "branch: main");
    }

    #[tokio::test]
    async fn runs_command_in_cwd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sentinel.txt"), "").unwrap();

        let expander = ShellExpander::new();
        let result = expander.expand("files: !`ls`", dir.path()).await.unwrap();
        assert!(result.contains("sentinel.txt"), "expected sentinel.txt in output: {result}");
    }

    #[tokio::test]
    async fn handles_multiple_commands() {
        let expander = ShellExpander::new();
        let cwd = std::env::current_dir().unwrap();
        let result = expander.expand("a=!`echo one`, b=!`echo two`", &cwd).await.unwrap();
        assert_eq!(result, "a=one, b=two");
    }

    #[tokio::test]
    async fn propagates_command_failure() {
        let expander = ShellExpander::new();
        let cwd = std::env::current_dir().unwrap();
        let err = expander.expand("!`exit 1`", &cwd).await.unwrap_err();
        let ShellExpansionError::NonZeroExit { cmd, .. } = err else {
            panic!("expected NonZeroExit, got {err:?}");
        };
        assert_eq!(cmd, "exit 1");
    }

    #[tokio::test]
    async fn trims_trailing_whitespace() {
        let expander = ShellExpander::new();
        let cwd = std::env::current_dir().unwrap();
        let result = expander.expand("!`printf 'hi\\n\\n'`", &cwd).await.unwrap();
        assert_eq!(result, "hi");
    }
}
