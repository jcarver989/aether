use crate::core::{AgentError, Result, substitute_parameters};
use mcp_utils::client::ServerInstructions;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub enum Prompt {
    Text(String),
    File {
        path: String,
        ancestors: bool,
        args: Option<HashMap<String, String>>,
        cwd: Option<PathBuf>,
    },
    SystemEnv(Option<PathBuf>),
    McpInstructions(Vec<ServerInstructions>),
}

impl Prompt {
    pub fn text(str: &str) -> Self {
        Self::Text(str.to_string())
    }

    pub fn file(path: &str, ancestors: bool) -> Self {
        Self::File {
            path: path.to_string(),
            ancestors,
            args: None,
            cwd: None,
        }
    }

    pub fn file_with_args(path: &str, ancestors: bool, args: HashMap<String, String>) -> Self {
        Self::File {
            path: path.to_string(),
            ancestors,
            args: Some(args),
            cwd: None,
        }
    }

    pub fn agents_md() -> Self {
        Self::File {
            path: "AGENTS.md".to_string(),
            ancestors: true,
            args: None,
            cwd: None,
        }
    }

    pub fn system_env() -> Self {
        Self::SystemEnv(None)
    }

    pub fn with_cwd(self, cwd: PathBuf) -> Self {
        match self {
            Self::File {
                path,
                ancestors,
                args,
                ..
            } => Self::File {
                path,
                ancestors,
                args,
                cwd: Some(cwd),
            },
            Self::SystemEnv(_) => Self::SystemEnv(Some(cwd)),
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
            Prompt::File {
                path,
                ancestors,
                args,
                cwd,
            } => {
                let content = if *ancestors {
                    Self::resolve_file_with_ancestors(path, cwd.as_deref()).await?
                } else {
                    Self::resolve_file(&PathBuf::from(path)).await?
                };

                Ok(substitute_parameters(&content, args))
            }
            Prompt::SystemEnv(cwd) => Self::resolve_system_env(cwd.as_deref()).await,
            Prompt::McpInstructions(instructions) => Ok(format_mcp_instructions(instructions)),
        }
    }

    /// Resolve multiple `SystemPrompts` and join them with double newlines
    pub async fn build_all(prompts: &[Prompt]) -> Result<String> {
        let mut parts = Vec::with_capacity(prompts.len());
        for p in prompts {
            parts.push(p.build().await?);
        }
        Ok(parts.join("\n\n"))
    }

    async fn resolve_file(path: &Path) -> Result<String> {
        fs::read_to_string(path).await.map_err(|e| {
            AgentError::IoError(format!("Failed to read file '{}': {e}", path.display()))
        })
    }

    async fn resolve_file_with_ancestors(filename: &str, cwd: Option<&Path>) -> Result<String> {
        let mut prompt = Vec::new();
        let mut current_dir = match cwd {
            Some(dir) => dir.to_path_buf(),
            None => env::current_dir().map_err(|e| {
                AgentError::IoError(format!("Failed to get current directory: {e}"))
            })?,
        };

        loop {
            let file_path = current_dir.join(filename);
            if fs::metadata(&file_path)
                .await
                .map(|m| m.is_file())
                .unwrap_or(false)
            {
                let content = Self::resolve_file(&file_path).await?;
                prompt.push(content);
            }

            match current_dir.parent() {
                Some(parent) => {
                    // Stop before root (/)
                    if parent.parent().is_none() {
                        break;
                    }
                    current_dir = parent.to_path_buf();
                }
                None => break,
            }
        }

        if prompt.is_empty() {
            return Err(AgentError::IoError(format!(
                "No '{filename}' files found in directory tree"
            )));
        }

        // Want root -> CWD (i.e. general --> specific prompt)
        prompt.reverse();
        Ok(prompt.join("\n\n"))
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

        let is_git_repo = if fs::metadata(cwd.join(".git"))
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            "Yes"
        } else {
            "No"
        };

        let mut lines = vec![
            format!("Working directory: {}", cwd.display()),
            format!("Platform: {}", env::consts::OS),
            format!("Today's date: {}", chrono::Local::now().format("%Y-%m-%d")),
            format!("Is directory a git repo: {}", is_git_repo),
        ];

        if let Some(os) = os_version {
            lines.push(os);
        }

        Ok(format!("<env>\n{}\n</env>", lines.join("\n")))
    }
}

/// Format MCP instructions with XML tags for the system prompt.
pub fn format_mcp_instructions(instructions: &[ServerInstructions]) -> String {
    if instructions.is_empty() {
        return String::new();
    }

    let mut parts = vec!["# MCP Server Instructions\n".to_string()];
    parts.push(
        "The following MCP servers have provided instructions for how to use their tools and resources:\n".to_string(),
    );

    for instr in instructions {
        parts.push(format!(
            "<mcp-server-instructions name=\"{}\">\n{}\n</mcp-server-instructions>",
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
        assert!(result.contains("Is directory a git repo:"));
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
    async fn resolve_file_with_ancestors_uses_provided_cwd() {
        let dir = std::env::temp_dir();
        let result = Prompt::resolve_file_with_ancestors("AGENTS.md", Some(dir.as_path())).await;
        // Should fail because no AGENTS.md in temp dir, but importantly it shouldn't
        // look in the process's cwd
        assert!(result.is_err());
    }
}
