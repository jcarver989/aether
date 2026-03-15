use crate::core::{AgentError, Result};
use glob::glob;
use mcp_utils::client::ServerInstructions;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;
use tracing::warn;
use utils::substitution::substitute_parameters;

#[derive(Debug, Clone)]
pub enum Prompt {
    Text(String),
    File {
        path: String,
        args: Option<HashMap<String, String>>,
        cwd: Option<PathBuf>,
    },
    /// Resolve prompt files from glob patterns relative to cwd.
    /// Absolute paths are also supported.
    PromptGlobs {
        patterns: Vec<String>,
        cwd: PathBuf,
    },
    SystemEnv(Option<PathBuf>),
    McpInstructions(Vec<ServerInstructions>),
}

impl Prompt {
    pub fn text(str: &str) -> Self {
        Self::Text(str.to_string())
    }

    pub fn file(path: &str) -> Self {
        Self::File {
            path: path.to_string(),
            args: None,
            cwd: None,
        }
    }

    pub fn file_with_args(path: &str, args: HashMap<String, String>) -> Self {
        Self::File {
            path: path.to_string(),
            args: Some(args),
            cwd: None,
        }
    }

    pub fn from_globs(patterns: Vec<String>, cwd: PathBuf) -> Self {
        Self::PromptGlobs { patterns, cwd }
    }

    pub fn system_env() -> Self {
        Self::SystemEnv(None)
    }

    pub fn with_cwd(self, cwd: PathBuf) -> Self {
        match self {
            Self::File { path, args, .. } => Self::File {
                path,
                args,
                cwd: Some(cwd),
            },
            Self::SystemEnv(_) => Self::SystemEnv(Some(cwd)),
            Self::PromptGlobs { patterns, .. } => Self::PromptGlobs { patterns, cwd },
            Self::Text(_) | Self::McpInstructions(_) => self,
        }
    }

    pub fn mcp_instructions(instructions: Vec<ServerInstructions>) -> Self {
        Self::McpInstructions(instructions)
    }

    /// Resolve this `SystemPrompt` to a String
    pub async fn build(&self) -> Result<String> {
        match self {
            Prompt::Text(text) => Ok(text.clone()),
            Prompt::File { path, args, cwd: _ } => {
                let content = Self::resolve_file(&PathBuf::from(path)).await?;
                Ok(substitute_parameters(&content, args))
            }
            Prompt::PromptGlobs { patterns, cwd } => {
                Self::resolve_prompt_globs(patterns, cwd).await
            }
            Prompt::SystemEnv(cwd) => Self::resolve_system_env(cwd.as_deref()).await,
            Prompt::McpInstructions(instructions) => Ok(format_mcp_instructions(instructions)),
        }
    }

    /// Resolve multiple `SystemPrompts` and join them with double newlines
    pub async fn build_all(prompts: &[Prompt]) -> Result<String> {
        let mut parts = Vec::with_capacity(prompts.len());
        for p in prompts {
            let part = p.build().await?;
            if !part.is_empty() {
                parts.push(part);
            }
        }
        Ok(parts.join("\n\n"))
    }

    async fn resolve_file(path: &Path) -> Result<String> {
        fs::read_to_string(path).await.map_err(|e| {
            AgentError::IoError(format!("Failed to read file '{}': {e}", path.display()))
        })
    }

    async fn resolve_prompt_globs(patterns: &[String], cwd: &Path) -> Result<String> {
        let mut contents = Vec::new();

        for pattern in patterns {
            let full_pattern = if Path::new(pattern).is_absolute() {
                pattern.clone()
            } else {
                cwd.join(pattern).to_string_lossy().to_string()
            };

            let paths = glob(&full_pattern).map_err(|e| {
                AgentError::IoError(format!("Invalid glob pattern '{pattern}': {e}"))
            })?;

            let mut matched: Vec<PathBuf> = paths.filter_map(std::result::Result::ok).collect();
            matched.sort();

            for path in matched {
                if path.is_file() {
                    match fs::read_to_string(&path).await {
                        Ok(content) => contents.push(content),
                        Err(e) => {
                            warn!("Failed to read prompt file '{}': {e}", path.display());
                        }
                    }
                }
            }
        }

        Ok(contents.join("\n\n"))
    }

    async fn resolve_system_env(cwd: Option<&Path>) -> Result<String> {
        let cwd = match cwd {
            Some(dir) => dir.to_path_buf(),
            None => env::current_dir().map_err(|e| {
                AgentError::IoError(format!("Failed to get current directory: {e}"))
            })?,
        };

        let os_version = Command::new("uname")
            .arg("-a")
            .output()
            .await
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|version| {
                let version = version.trim();
                if version.is_empty() {
                    None
                } else {
                    Some(format!("OS Version: {version}"))
                }
            });

        let is_git_repo = fs::metadata(cwd.join(".git"))
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false);

        let working_dir = if is_git_repo {
            format!("Working directory: {} (git repo)", cwd.display())
        } else {
            format!("Working directory: {}", cwd.display())
        };

        let mut lines = vec![
            working_dir,
            format!("Platform: {}", env::consts::OS),
            format!("Today's date: {}", chrono::Local::now().format("%Y-%m-%d")),
        ];

        if let Some(os) = os_version {
            lines.push(os);
        }

        Ok(format!("<env>\n{}\n</env>", lines.join("\n")))
    }
}

/// Format MCP instructions with XML tags for the system prompt.
fn format_mcp_instructions(instructions: &[ServerInstructions]) -> String {
    if instructions.is_empty() {
        return String::new();
    }

    let mut parts = vec!["# MCP Server Instructions\n".to_string()];
    parts.push(
        "The following MCP servers have provided instructions for how to use their tools and resources:\n".to_string(),
    );

    for instr in instructions {
        parts.push(format!(
            "<mcp-server-instructions name=\"{}\">\n{}\n</mcp-server-instructions>\n",
            instr.server_name, instr.instructions
        ));
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn build_text_prompt() {
        let prompt = Prompt::text("Hello, world!");
        let result = prompt.build().await.unwrap();
        assert_eq!(result, "Hello, world!");
    }

    #[tokio::test]
    async fn build_all_concatenates_prompts() {
        let prompts = vec![Prompt::text("Part one"), Prompt::text("Part two")];
        let result = Prompt::build_all(&prompts).await.unwrap();
        assert_eq!(result, "Part one\n\nPart two");
    }

    #[tokio::test]
    async fn resolve_system_env_contains_expected_fields() {
        let result = Prompt::resolve_system_env(None).await.unwrap();
        assert!(result.contains("<env>"));
        assert!(result.contains("</env>"));
        assert!(result.contains("Working directory:"));
        assert!(result.contains("Platform:"));
        assert!(result.contains("Today's date:"));
    }

    #[tokio::test]
    async fn resolve_system_env_uses_provided_cwd() {
        let cwd = std::env::temp_dir();
        let result = Prompt::resolve_system_env(Some(cwd.as_path()))
            .await
            .unwrap();
        assert!(result.contains(&cwd.display().to_string()));
    }

    #[tokio::test]
    async fn prompt_globs_resolves_single_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "# Instructions\nBe helpful").unwrap();

        let prompt = Prompt::from_globs(vec!["AGENTS.md".to_string()], dir.path().to_path_buf());
        let result = prompt.build().await.unwrap();
        assert_eq!(result, "# Instructions\nBe helpful");
    }

    #[tokio::test]
    async fn prompt_globs_resolves_glob_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let rules_dir = dir.path().join(".aether/rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("a-coding.md"), "Use Rust").unwrap();
        std::fs::write(rules_dir.join("b-testing.md"), "Write tests").unwrap();

        let prompt = Prompt::from_globs(
            vec![".aether/rules/*.md".to_string()],
            dir.path().to_path_buf(),
        );
        let result = prompt.build().await.unwrap();
        assert!(result.contains("Use Rust"));
        assert!(result.contains("Write tests"));
    }

    #[tokio::test]
    async fn prompt_globs_returns_empty_for_no_matches() {
        let dir = tempfile::tempdir().unwrap();

        let prompt = Prompt::from_globs(
            vec!["nonexistent*.md".to_string()],
            dir.path().to_path_buf(),
        );
        let result = prompt.build().await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn prompt_globs_supports_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("rules.md");
        std::fs::write(&file_path, "Absolute rule").unwrap();

        let prompt = Prompt::from_globs(
            vec![file_path.to_string_lossy().to_string()],
            PathBuf::from("/tmp"),
        );
        let result = prompt.build().await.unwrap();
        assert_eq!(result, "Absolute rule");
    }

    #[tokio::test]
    async fn prompt_globs_concatenates_multiple_patterns() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "Agent instructions").unwrap();
        std::fs::write(dir.path().join("SYSTEM.md"), "System prompt").unwrap();

        let prompt = Prompt::from_globs(
            vec!["AGENTS.md".to_string(), "SYSTEM.md".to_string()],
            dir.path().to_path_buf(),
        );
        let result = prompt.build().await.unwrap();
        assert!(result.contains("Agent instructions"));
        assert!(result.contains("System prompt"));
        assert!(result.contains("\n\n"));
    }

    #[tokio::test]
    async fn build_all_skips_empty_parts() {
        let prompts = vec![
            Prompt::text("Part one"),
            Prompt::text(""),
            Prompt::text("Part two"),
        ];
        let result = Prompt::build_all(&prompts).await.unwrap();
        assert_eq!(result, "Part one\n\nPart two");
    }
}
