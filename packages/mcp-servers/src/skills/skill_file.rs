use std::fs::{self, read_dir, read_to_string};
use std::path::Path;

use mcp_utils::MarkdownFile;
use serde::{Deserialize, Serialize};

pub const SKILL_FILENAME: &str = "SKILL.md";

pub type SkillsFile = MarkdownFile<SkillsFrontmatter>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillsFrontmatter {
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub agent_authored: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub helpful: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub harmful: u32,
}

impl SkillsFrontmatter {
    pub fn confidence(&self) -> f64 {
        f64::from(self.helpful) / (f64::from(self.helpful) + f64::from(self.harmful) + 1.0)
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub agent_authored: bool,
}

impl SkillMetadata {
    pub fn from_dir(dir_path: &Path) -> Option<Self> {
        let skill_file_path = dir_path.join(SKILL_FILENAME);
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
        let frontmatter = if let Some(rest) = content.strip_prefix("---") {
            let end_pos = rest.find("\n---")?;
            let frontmatter_str = &rest[..end_pos];
            serde_yml::from_str::<SkillsFrontmatter>(frontmatter_str).ok()
        } else {
            None
        };

        let name = dir_path.file_name()?.to_string_lossy().to_string();

        Some(SkillMetadata {
            description: frontmatter
                .as_ref()
                .map(|f| f.description.clone())
                .unwrap_or_default(),
            tags: frontmatter
                .as_ref()
                .map(|f| f.tags.clone())
                .unwrap_or_default(),
            agent_authored: frontmatter.as_ref().is_some_and(|f| f.agent_authored),
            name,
        })
    }
}

/// Read and parse a SKILL.md file, returning frontmatter and body content.
pub fn read_and_parse(dir: &Path) -> Result<(SkillsFrontmatter, String), SkillFileError> {
    let path = dir.join(SKILL_FILENAME);
    let raw = fs::read_to_string(&path)?;
    let trimmed = raw.trim();

    let rest = trimmed
        .strip_prefix("---")
        .ok_or(SkillFileError::MissingFrontmatter)?;

    let end_pos = rest
        .find("\n---")
        .ok_or(SkillFileError::MissingFrontmatter)?;

    let frontmatter_str = &rest[..end_pos];
    let body = rest[end_pos + 4..].trim().to_string();

    let frontmatter: SkillsFrontmatter =
        serde_yml::from_str(frontmatter_str).map_err(|e| SkillFileError::Yaml(e.to_string()))?;

    Ok((frontmatter, body))
}

/// Write a SKILL.md file with frontmatter and body content.
pub fn write_skill(
    dir: &Path,
    frontmatter: &SkillsFrontmatter,
    body: &str,
) -> Result<(), SkillFileError> {
    fs::create_dir_all(dir)?;

    let yaml =
        serde_yml::to_string(frontmatter).map_err(|e| SkillFileError::Yaml(e.to_string()))?;

    let file_content = if body.is_empty() {
        format!("---\n{yaml}---\n")
    } else {
        format!("---\n{yaml}---\n{body}\n")
    };
    fs::write(dir.join(SKILL_FILENAME), file_content)?;
    Ok(())
}

/// Check if a SKILL.md file exists in the given directory.
pub fn skill_exists(dir: &Path) -> bool {
    dir.join(SKILL_FILENAME).exists()
}

pub fn load_skill_metadata(skills_dir: &Path) -> Vec<SkillMetadata> {
    if !skills_dir.exists() || !skills_dir.is_dir() {
        return Vec::new();
    }

    read_dir(skills_dir)
        .inspect_err(|e| tracing::warn!("Failed to read skills directory: {e}"))
        .ok()
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .filter(|e| {
                    let path = e.path();
                    path.is_dir() && !e.file_name().to_string_lossy().starts_with('.')
                })
                .filter_map(|entry| SkillMetadata::from_dir(&entry.path()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[derive(Debug)]
pub enum SkillFileError {
    Io(std::io::Error),
    Yaml(String),
    MissingFrontmatter,
    NotFound(String),
    NotAgentAuthored(String),
}

impl std::fmt::Display for SkillFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillFileError::Io(e) => write!(f, "IO error: {e}"),
            SkillFileError::Yaml(e) => write!(f, "YAML error: {e}"),
            SkillFileError::MissingFrontmatter => write!(f, "missing YAML frontmatter"),
            SkillFileError::NotFound(name) => write!(f, "skill not found: {name}"),
            SkillFileError::NotAgentAuthored(name) => {
                write!(
                    f,
                    "skill '{name}' is not agent-authored and cannot be modified"
                )
            }
        }
    }
}

impl std::error::Error for SkillFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SkillFileError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SkillFileError {
    fn from(e: std::io::Error) -> Self {
        SkillFileError::Io(e)
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip_serializing_if requires &T
fn is_false(b: &bool) -> bool {
    !(*b)
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip_serializing_if requires &T
fn is_zero(n: &u32) -> bool {
    *n == 0
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
        create_skill_file(&temp_dir, "test-skill", "A test skill", &[]);

        let result = load_skill_metadata(temp_dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "test-skill");
        assert_eq!(result[0].description, "A test skill");
        assert!(result[0].tags.is_empty());
        assert!(!result[0].agent_authored);
    }

    #[test]
    fn test_load_skill_metadata_with_tags() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("tagged-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join(SKILL_FILENAME),
            "---\ndescription: A tagged skill\ntags:\n  - convention\n  - testing\n---\n# Content\n",
        )
        .unwrap();

        let result = load_skill_metadata(temp_dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tags, vec!["convention", "testing"]);
    }

    #[test]
    fn test_load_skill_metadata_agent_authored() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("agent-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join(SKILL_FILENAME),
            "---\ndescription: An agent skill\nagent_authored: true\n---\n# Content\n",
        )
        .unwrap();

        let result = load_skill_metadata(temp_dir.path());
        assert_eq!(result.len(), 1);
        assert!(result[0].agent_authored);
    }

    #[test]
    fn test_load_skill_metadata_multiple_skills() {
        let temp_dir = TempDir::new().unwrap();
        create_skill_file(&temp_dir, "skill-1", "First skill", &[]);
        create_skill_file(&temp_dir, "skill-2", "Second skill", &[]);
        create_skill_file(&temp_dir, "skill-3", "Third skill", &[]);

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
    fn test_load_skill_metadata_skips_hidden_directories() {
        let temp_dir = TempDir::new().unwrap();
        create_skill_file(&temp_dir, ".archived", "Hidden skill", &[]);
        create_skill_file(&temp_dir, "visible-skill", "Visible skill", &[]);

        let result = load_skill_metadata(temp_dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "visible-skill");
    }

    #[test]
    fn test_skill_info_from_dir_without_frontmatter() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("no-frontmatter");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join(SKILL_FILENAME), "# No frontmatter").unwrap();

        let result = SkillMetadata::from_dir(&skill_dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap().description, "");
    }

    #[test]
    fn test_skill_info_from_dir_nonexistent() {
        let result = SkillMetadata::from_dir(PathBuf::from("/nonexistent").as_path());
        assert!(result.is_none());
    }

    #[test]
    fn test_frontmatter_serde_roundtrip() {
        let fm = SkillsFrontmatter {
            description: "A simple skill".to_string(),
            tags: vec![],
            agent_authored: false,
            helpful: 0,
            harmful: 0,
        };

        let yaml = serde_yml::to_string(&fm).unwrap();
        let parsed: SkillsFrontmatter = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed.description, "A simple skill");
        assert!(parsed.tags.is_empty());
        assert!(!parsed.agent_authored);
    }

    #[test]
    fn test_frontmatter_serde_with_all_fields() {
        let fm = SkillsFrontmatter {
            description: "A full skill".to_string(),
            tags: vec!["convention".to_string(), "testing".to_string()],
            agent_authored: true,
            helpful: 5,
            harmful: 2,
        };

        let yaml = serde_yml::to_string(&fm).unwrap();
        let parsed: SkillsFrontmatter = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed.description, "A full skill");
        assert_eq!(parsed.tags, vec!["convention", "testing"]);
        assert!(parsed.agent_authored);
        assert_eq!(parsed.helpful, 5);
        assert_eq!(parsed.harmful, 2);
    }

    #[test]
    fn test_backward_compat_old_frontmatter() {
        let yaml = "description: An old skill\n";
        let parsed: SkillsFrontmatter = serde_yml::from_str(yaml).unwrap();
        assert_eq!(parsed.description, "An old skill");
        assert!(parsed.tags.is_empty());
        assert!(!parsed.agent_authored);
        assert_eq!(parsed.helpful, 0);
        assert_eq!(parsed.harmful, 0);
    }

    #[test]
    fn test_confidence() {
        let fm = |helpful, harmful| SkillsFrontmatter {
            description: String::new(),
            tags: vec![],
            agent_authored: true,
            helpful,
            harmful,
        };

        assert!((fm(0, 0).confidence() - 0.0).abs() < f64::EPSILON);
        assert!((fm(7, 1).confidence() - 7.0 / 9.0).abs() < f64::EPSILON);
        assert!((fm(0, 5).confidence() - 0.0).abs() < f64::EPSILON);
        assert!((fm(3, 0).confidence() - 3.0 / 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_read_and_parse() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("my-skill");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(SKILL_FILENAME),
            "---\ndescription: Test skill\ntags:\n  - rust\nagent_authored: true\nhelpful: 3\nharmful: 1\n---\n# My Skill\n\nSome content here.\n",
        ).unwrap();

        let (fm, body) = read_and_parse(&dir).unwrap();
        assert_eq!(fm.description, "Test skill");
        assert_eq!(fm.tags, vec!["rust"]);
        assert!(fm.agent_authored);
        assert_eq!(fm.helpful, 3);
        assert_eq!(fm.harmful, 1);
        assert!(body.contains("# My Skill"));
        assert!(body.contains("Some content here."));
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("my-skill");

        let fm = SkillsFrontmatter {
            description: "Test skill".to_string(),
            tags: vec!["convention".to_string()],
            agent_authored: true,
            helpful: 2,
            harmful: 1,
        };
        write_skill(&dir, &fm, "# My Skill\n\nSome content here.").unwrap();

        let (fm2, body) = read_and_parse(&dir).unwrap();
        assert_eq!(fm2.description, "Test skill");
        assert_eq!(fm2.tags, vec!["convention"]);
        assert!(fm2.agent_authored);
        assert_eq!(fm2.helpful, 2);
        assert_eq!(fm2.harmful, 1);
        assert!(body.contains("# My Skill"));
        assert!(body.contains("Some content here."));
    }

    #[test]
    fn test_write_empty_body() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("empty-body");

        let fm = SkillsFrontmatter {
            description: "Empty".to_string(),
            tags: vec![],
            agent_authored: true,
            helpful: 0,
            harmful: 0,
        };
        write_skill(&dir, &fm, "").unwrap();

        let raw = std::fs::read_to_string(dir.join(SKILL_FILENAME)).unwrap();
        assert!(raw.starts_with("---\n"));
        assert!(raw.contains("description: Empty"));
    }

    #[test]
    fn test_skill_exists() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("check");
        assert!(!skill_exists(&dir));

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SKILL_FILENAME), "---\ndescription: x\n---\n").unwrap();
        assert!(skill_exists(&dir));
    }

    #[test]
    fn test_skip_serializing_defaults() {
        let fm = SkillsFrontmatter {
            description: "Minimal".to_string(),
            tags: vec![],
            agent_authored: false,
            helpful: 0,
            harmful: 0,
        };

        let yaml = serde_yml::to_string(&fm).unwrap();
        assert!(!yaml.contains("tags"));
        assert!(!yaml.contains("agent_authored"));
        assert!(!yaml.contains("helpful"));
        assert!(!yaml.contains("harmful"));
    }

    fn create_skill_file(temp_dir: &TempDir, name: &str, description: &str, tags: &[&str]) {
        let skill_dir = temp_dir.path().join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let tags_yaml = if tags.is_empty() {
            String::new()
        } else {
            let tag_list: Vec<String> = tags.iter().map(|t| format!("  - {t}")).collect();
            format!("tags:\n{}\n", tag_list.join("\n"))
        };
        let content = format!("---\ndescription: {description}\n{tags_yaml}---\n# Skill Content\n");
        std::fs::write(skill_dir.join(SKILL_FILENAME), content).unwrap();
    }
}
