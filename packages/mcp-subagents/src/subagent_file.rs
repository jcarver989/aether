use mcp_utils::MarkdownFile;
use serde::{Deserialize, Serialize};
use std::fs::{read_dir, read_to_string};
use std::path::Path;

pub type AgentFile = MarkdownFile<AgentFrontmatter>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    pub description: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct SubAgentInfo {
    pub name: String,
    pub description: String,
}

impl SubAgentInfo {
    /// Load sub-agent metadata from a directory containing an AGENTS.md file.
    /// Returns None if the directory doesn't exist or can't be read.
    pub fn from_dir(dir_path: &Path) -> Option<Self> {
        let agent_file_path = dir_path.join("AGENTS.md");
        let raw_content = read_to_string(&agent_file_path)
            .inspect_err(|e| {
                tracing::warn!(
                    "Failed to read agent file {}: {}",
                    agent_file_path.display(),
                    e
                );
            })
            .ok()?;

        let content = raw_content.trim();
        let description = if let Some(rest) = content.strip_prefix("---") {
            let end_pos = rest.find("\n---")?;
            let frontmatter_str = &rest[..end_pos];
            serde_yaml::from_str::<AgentFrontmatter>(frontmatter_str)
                .ok()
                .map(|f| f.description)
                .unwrap_or_default()
        } else {
            String::new()
        };

        let name = dir_path.file_name()?.to_string_lossy().to_string();

        Some(SubAgentInfo { name, description })
    }
}

/// Load sub-agent metadata from the agents directory synchronously.
/// This is used to build the MCP server instructions at startup.
/// Returns an empty vec if the directory doesn't exist or can't be read.
pub fn load_agent_metadata(agents_dir: &Path) -> Vec<SubAgentInfo> {
    if !agents_dir.exists() || !agents_dir.is_dir() {
        return Vec::new();
    }

    read_dir(agents_dir)
        .inspect_err(|e| tracing::warn!("Failed to read agents directory: {e}"))
        .ok()
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .filter(|e| e.path().is_dir())
                .filter_map(|entry| SubAgentInfo::from_dir(&entry.path()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_load_agent_metadata_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_agent_metadata(temp_dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_agent_metadata_nonexistent_directory() {
        let result = load_agent_metadata(&PathBuf::from("/nonexistent/path"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_agent_metadata_single_agent() {
        let temp_dir = TempDir::new().unwrap();
        create_agent_file(&temp_dir, "test-agent", "A test agent", "claude-3-5-sonnet");

        let result = load_agent_metadata(temp_dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "test-agent");
        assert_eq!(result[0].description, "A test agent");
    }

    #[test]
    fn test_load_agent_metadata_multiple_agents() {
        let temp_dir = TempDir::new().unwrap();
        create_agent_file(&temp_dir, "agent-1", "First agent", "claude-3-5-sonnet");
        create_agent_file(&temp_dir, "agent-2", "Second agent", "gpt-4");
        create_agent_file(&temp_dir, "agent-3", "Third agent", "claude-3-haiku");

        let result = load_agent_metadata(temp_dir.path());
        assert_eq!(result.len(), 3);

        let names: Vec<_> = result.iter().map(|a| a.name.clone()).collect();
        assert!(names.contains(&"agent-1".to_string()));
        assert!(names.contains(&"agent-2".to_string()));
        assert!(names.contains(&"agent-3".to_string()));
    }

    #[test]
    fn test_load_agent_metadata_skips_directories_without_agents_md() {
        let temp_dir = TempDir::new().unwrap();
        let agent_dir = temp_dir.path().join("empty-agent");
        std::fs::create_dir_all(&agent_dir).unwrap();

        let result = load_agent_metadata(temp_dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_subagent_info_from_dir_without_frontmatter() {
        let temp_dir = TempDir::new().unwrap();
        let agent_dir = temp_dir.path().join("no-frontmatter");
        std::fs::create_dir_all(&agent_dir).unwrap();
        std::fs::write(agent_dir.join("AGENTS.md"), "# No frontmatter").unwrap();

        let result = SubAgentInfo::from_dir(&agent_dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap().description, "");
    }

    #[test]
    fn test_subagent_info_from_dir_nonexistent() {
        let result = SubAgentInfo::from_dir(PathBuf::from("/nonexistent").as_path());
        assert!(result.is_none());
    }

    fn create_agent_file(temp_dir: &TempDir, name: &str, description: &str, model: &str) {
        let agent_dir = temp_dir.path().join(name);
        std::fs::create_dir_all(&agent_dir).unwrap();
        let content = format!(
            "---\ndescription: {} \nmodel: {} \n---\n# Agent Content\n",
            description, model
        );
        std::fs::write(agent_dir.join("AGENTS.md"), content).unwrap();
    }
}
