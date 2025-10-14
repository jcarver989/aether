use crate::agent::{AgentError, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum Prompt {
    Text(String),
    File { path: String, ancestors: bool },
}

impl Prompt {
    pub fn text(str: &str) -> Self {
        Self::Text(str.to_string())
    }

    pub fn file(path: &str, ancestors: bool) -> Self {
        Self::File {
            path: path.to_string(),
            ancestors,
        }
    }

    pub fn agents_md() -> Self {
        Self::File {
            path: "AGENTS.md".to_string(),
            ancestors: true,
        }
    }

    /// Resolve this SystemPrompt to a String
    pub fn build(&self) -> Result<String> {
        match self {
            Prompt::Text(text) => Ok(text.clone()),
            Prompt::File { path, ancestors } => {
                if *ancestors {
                    Self::resolve_file_with_ancestors(path)
                } else {
                    Self::resolve_file(&PathBuf::from(path))
                }
            }
        }
    }

    /// Resolve multiple SystemPrompts and join them with double newlines
    pub fn build_all(prompts: &[Prompt]) -> Result<String> {
        let content: Result<Vec<_>> = prompts.iter().map(|p| p.build()).collect();
        Ok(content?.join("\n\n"))
    }

    fn resolve_file(path: &Path) -> Result<String> {
        fs::read_to_string(path).map_err(|e| {
            AgentError::IoError(format!("Failed to read file '{}': {e}", path.display()))
        })
    }

    fn resolve_file_with_ancestors(filename: &str) -> Result<String> {
        let mut prompt = Vec::new();
        let mut current_dir = env::current_dir()
            .map_err(|e| AgentError::IoError(format!("Failed to get current directory: {e}")))?;

        loop {
            let file_path = current_dir.join(filename);
            if file_path.exists() && file_path.is_file() {
                let content = Self::resolve_file(&file_path)?;
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
}
