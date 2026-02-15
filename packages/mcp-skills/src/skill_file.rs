use std::fs::{read_dir, read_to_string};
use std::path::Path;

use mcp_utils::MarkdownFile;
use serde::{Deserialize, Serialize};

pub type SkillsFile = MarkdownFile<SkillsFrontmatter>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsFrontmatter {
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
}

impl SkillInfo {
    /// Load skill metadata from a directory containing a SKILL.md file.
    /// Returns None if the directory doesn't exist or can't be read.
    pub fn from_dir(dir_path: &Path) -> Option<Self> {
        let skill_file_path = dir_path.join("SKILL.md");
        let raw_content = read_to_string(&skill_file_path)
            .inspect_err(|e| {
                tracing::warn!(
                    "Failed to read skill file {}: {}",
                    skill_file_path.display(),
                    e
                );
            })
            .ok()?;

        let content = raw_content.trim();
        let description = if let Some(rest) = content.strip_prefix("---") {
            let end_pos = rest.find("\n---")?;
            let frontmatter_str = &rest[..end_pos];
            serde_yaml::from_str::<SkillsFrontmatter>(frontmatter_str)
                .ok()
                .map(|f| f.description)
                .unwrap_or_default()
        } else {
            String::new()
        };

        let name = dir_path.file_name()?.to_string_lossy().to_string();

        Some(SkillInfo { name, description })
    }
}

/// Load skill metadata from the skills directory synchronously.
/// This is used to build the MCP server instructions at startup.
/// Returns an empty vec if the directory doesn't exist or can't be read.
pub fn load_skill_metadata(skills_dir: &Path) -> Vec<SkillInfo> {
    if !skills_dir.exists() || !skills_dir.is_dir() {
        return Vec::new();
    }

    read_dir(skills_dir)
        .inspect_err(|e| tracing::warn!("Failed to read skills directory: {e}"))
        .ok()
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|entry| SkillInfo::from_dir(&entry.path()))
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
    fn test_load_skill_metadata_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_skill_metadata(temp_dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_skill_metadata_nonexistent_directory() {
        let result = load_skill_metadata(&PathBuf::from("/nonexistent/path"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_skill_metadata_single_skill() {
        let temp_dir = TempDir::new().unwrap();
        create_skill_file(&temp_dir, "test-skill", "A test skill");

        let result = load_skill_metadata(temp_dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "test-skill");
        assert_eq!(result[0].description, "A test skill");
    }

    #[test]
    fn test_load_skill_metadata_multiple_skills() {
        let temp_dir = TempDir::new().unwrap();
        create_skill_file(&temp_dir, "skill-1", "First skill");
        create_skill_file(&temp_dir, "skill-2", "Second skill");
        create_skill_file(&temp_dir, "skill-3", "Third skill");

        let result = load_skill_metadata(temp_dir.path());
        assert_eq!(result.len(), 3);

        let names: Vec<_> = result.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"skill-1".to_string()));
        assert!(names.contains(&"skill-2".to_string()));
        assert!(names.contains(&"skill-3".to_string()));
    }

    #[test]
    fn test_load_skill_metadata_skips_directories_without_skill_md() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("empty-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let result = load_skill_metadata(temp_dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_skill_info_from_dir_without_frontmatter() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("no-frontmatter");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# No frontmatter").unwrap();

        let result = SkillInfo::from_dir(&skill_dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap().description, "");
    }

    #[test]
    fn test_skill_info_from_dir_nonexistent() {
        let result = SkillInfo::from_dir(PathBuf::from("/nonexistent").as_path());
        assert!(result.is_none());
    }

    fn create_skill_file(temp_dir: &TempDir, name: &str, description: &str) {
        let skill_dir = temp_dir.path().join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let content = format!("---\ndescription: {} \n---\n# Skill Content\n", description);
        std::fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }
}
