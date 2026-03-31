use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

pub const SKILL_FILENAME: &str = "SKILL.md";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct PromptFrontmatter {
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, rename = "user-invocable", skip_serializing_if = "not")]
    pub user_invocable: bool,
    #[serde(default, rename = "agent-invocable", skip_serializing_if = "not")]
    pub agent_invocable: bool,
    #[serde(default, rename = "argument-hint", skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triggers: Option<Triggers>,
    #[serde(default, skip_serializing_if = "not")]
    pub agent_authored: bool,
    #[serde(default, skip_serializing_if = "zero")]
    pub helpful: u32,
    #[serde(default, skip_serializing_if = "zero")]
    pub harmful: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Triggers {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read: Vec<String>,
}

/// A resolved skill artifact discovered from a `SKILL.md` file.
#[derive(Debug, Clone)]
pub struct PromptFile {
    pub name: String,
    pub description: String,
    pub body: String,
    pub path: PathBuf,
    pub user_invocable: bool,
    pub agent_invocable: bool,
    pub argument_hint: Option<String>,
    pub tags: Vec<String>,
    pub triggers: PromptTriggers,
    pub agent_authored: bool,
    pub helpful: u32,
    pub harmful: u32,
}

impl PromptFile {
    /// Parse a prompt file at the given path into a `PromptFile`.
    ///
    /// The name defaults to the parent directory name unless overridden in frontmatter.
    pub fn parse(path: &Path) -> Result<Self, PromptFileError> {
        let raw = fs::read_to_string(path)?;

        let (frontmatter, body) = Self::parse_frontmatter(raw.trim())?;

        let dir_name =
            path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

        let name = frontmatter.name.unwrap_or(dir_name);
        let description = frontmatter.description.trim().to_string();

        if description.is_empty() {
            return Err(PromptFileError::MissingDescription { name });
        }

        let has_read_triggers = frontmatter.triggers.as_ref().is_some_and(|t| !t.read.is_empty());

        if !frontmatter.user_invocable && !frontmatter.agent_invocable && !has_read_triggers {
            return Err(PromptFileError::NoActivationSurface { name });
        }

        let read_globs = frontmatter.triggers.map(|t| t.read).unwrap_or_default();
        let triggers = PromptTriggers::new(read_globs)?;

        Ok(Self {
            name,
            description,
            body,
            path: path.to_path_buf(),
            user_invocable: frontmatter.user_invocable,
            agent_invocable: frontmatter.agent_invocable,
            argument_hint: frontmatter.argument_hint,
            tags: frontmatter.tags,
            triggers,
            agent_authored: frontmatter.agent_authored,
            helpful: frontmatter.helpful,
            harmful: frontmatter.harmful,
        })
    }

    /// Validate this prompt file has a non-empty description and at least one activation surface.
    pub fn validate(&self) -> Result<(), PromptFileError> {
        if self.description.trim().is_empty() {
            return Err(PromptFileError::MissingDescription { name: self.name.clone() });
        }

        let has_read_triggers = !self.triggers.is_empty();
        if !self.user_invocable && !self.agent_invocable && !has_read_triggers {
            return Err(PromptFileError::NoActivationSurface { name: self.name.clone() });
        }

        Ok(())
    }

    /// Write this prompt file to the given path, creating parent directories as needed.
    pub fn write(&self, path: &Path) -> Result<(), PromptFileError> {
        self.validate()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let triggers =
            if self.triggers.is_empty() { None } else { Some(Triggers { read: self.triggers.patterns().to_vec() }) };

        let frontmatter = PromptFrontmatter {
            description: self.description.clone(),
            name: Some(self.name.clone()),
            user_invocable: self.user_invocable,
            agent_invocable: self.agent_invocable,
            argument_hint: self.argument_hint.clone(),
            tags: self.tags.clone(),
            triggers,
            agent_authored: self.agent_authored,
            helpful: self.helpful,
            harmful: self.harmful,
        };

        let yaml = serde_yml::to_string(&frontmatter).map_err(|e| PromptFileError::Yaml(e.to_string()))?;

        let file_content =
            if self.body.is_empty() { format!("---\n{yaml}---\n") } else { format!("---\n{yaml}---\n{}\n", self.body) };
        fs::write(path, file_content)?;
        Ok(())
    }

    /// Confidence score based on helpful/harmful ratings.
    pub fn confidence(&self) -> f64 {
        f64::from(self.helpful) / (f64::from(self.helpful) + f64::from(self.harmful) + 1.0)
    }

    /// Parse YAML frontmatter and body from a SKILL.md content string (no I/O).
    fn parse_frontmatter(content: &str) -> Result<(PromptFrontmatter, String), PromptFileError> {
        let (yaml_str, body) =
            utils::markdown_file::split_frontmatter(content).ok_or(PromptFileError::MissingFrontmatter)?;

        let frontmatter: PromptFrontmatter =
            serde_yml::from_str(yaml_str).map_err(|e| PromptFileError::Yaml(e.to_string()))?;

        Ok((frontmatter, body.to_string()))
    }
}

/// Trigger configuration for automatic prompt activation.
#[derive(Debug, Clone, Default)]
pub struct PromptTriggers {
    patterns: Vec<String>,
    globs: Option<GlobSet>,
}

impl PromptTriggers {
    fn new(glob_patterns: Vec<String>) -> Result<Self, PromptFileError> {
        if glob_patterns.is_empty() {
            return Ok(Self { patterns: Vec::new(), globs: None });
        }

        let mut builder = GlobSetBuilder::new();
        for pattern in &glob_patterns {
            let glob = Glob::new(pattern)
                .map_err(|e| PromptFileError::InvalidTriggerGlob { pattern: pattern.clone(), error: e.to_string() })?;
            builder.add(glob);
        }

        let globs = builder.build().map_err(|e| PromptFileError::InvalidTriggerGlob {
            pattern: glob_patterns.join(", "),
            error: e.to_string(),
        })?;

        Ok(Self { patterns: glob_patterns, globs: Some(globs) })
    }

    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    pub fn is_empty(&self) -> bool {
        self.globs.is_none()
    }

    /// Check if a project-relative path matches any read trigger glob.
    pub fn matches_read(&self, relative_path: &str) -> bool {
        self.globs.as_ref().is_some_and(|gs| gs.is_match(relative_path))
    }
}

#[derive(Debug)]
pub enum PromptFileError {
    Io(std::io::Error),
    Yaml(String),
    MissingFrontmatter,
    MissingDescription { name: String },
    NoActivationSurface { name: String },
    InvalidTriggerGlob { pattern: String, error: String },
    NotFound(String),
    NotAgentAuthored(String),
}

impl Display for PromptFileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptFileError::Io(e) => write!(f, "IO error: {e}"),
            PromptFileError::Yaml(e) => write!(f, "YAML error: {e}"),
            PromptFileError::MissingFrontmatter => write!(f, "missing YAML frontmatter"),
            PromptFileError::MissingDescription { name } => {
                write!(f, "skill '{name}' has an empty description")
            }
            PromptFileError::NoActivationSurface { name } => {
                write!(f, "skill '{name}' must have at least one of: user-invocable, agent-invocable, or triggers.read")
            }
            PromptFileError::InvalidTriggerGlob { pattern, error } => {
                write!(f, "invalid trigger glob '{pattern}': {error}")
            }
            PromptFileError::NotFound(name) => write!(f, "skill not found: {name}"),
            PromptFileError::NotAgentAuthored(name) => {
                write!(f, "skill '{name}' is not agent-authored and cannot be modified")
            }
        }
    }
}

impl std::error::Error for PromptFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PromptFileError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PromptFileError {
    fn from(e: std::io::Error) -> Self {
        PromptFileError::Io(e)
    }
}

#[expect(clippy::trivially_copy_pass_by_ref)]
fn not(b: &bool) -> bool {
    !b
}

#[expect(clippy::trivially_copy_pass_by_ref)]
fn zero(n: &u32) -> bool {
    *n == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn minimal_frontmatter(description: &str) -> PromptFrontmatter {
        PromptFrontmatter {
            description: description.to_string(),
            name: None,
            user_invocable: false,
            agent_invocable: false,
            argument_hint: None,
            tags: vec![],
            triggers: None,
            agent_authored: false,
            helpful: 0,
            harmful: 0,
        }
    }

    #[test]
    fn frontmatter_serde_roundtrip() {
        let fm = minimal_frontmatter("A simple skill");

        let yaml = serde_yml::to_string(&fm).unwrap();
        let parsed: PromptFrontmatter = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed.description, "A simple skill");
        assert!(parsed.tags.is_empty());
        assert!(!parsed.agent_authored);
    }

    #[test]
    fn frontmatter_serde_with_all_fields() {
        let mut fm = minimal_frontmatter("A full skill");
        fm.tags = vec!["convention".to_string(), "testing".to_string()];
        fm.agent_authored = true;
        fm.helpful = 5;
        fm.harmful = 2;

        let yaml = serde_yml::to_string(&fm).unwrap();
        let parsed: PromptFrontmatter = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed.description, "A full skill");
        assert_eq!(parsed.tags, vec!["convention", "testing"]);
        assert!(parsed.agent_authored);
        assert_eq!(parsed.helpful, 5);
        assert_eq!(parsed.harmful, 2);
    }

    #[test]
    fn backward_compat_old_frontmatter() {
        let yaml = "description: An old skill\n";
        let parsed: PromptFrontmatter = serde_yml::from_str(yaml).unwrap();
        assert_eq!(parsed.description, "An old skill");
        assert!(parsed.tags.is_empty());
        assert!(!parsed.agent_authored);
        assert_eq!(parsed.helpful, 0);
        assert_eq!(parsed.harmful, 0);
    }

    #[test]
    fn confidence() {
        let pf = |helpful, harmful| PromptFile {
            name: String::new(),
            description: "test".to_string(),
            body: String::new(),
            path: PathBuf::new(),
            user_invocable: false,
            agent_invocable: false,
            argument_hint: None,
            tags: vec![],
            triggers: PromptTriggers::default(),
            agent_authored: true,
            helpful,
            harmful,
        };

        assert!((pf(0, 0).confidence() - 0.0).abs() < f64::EPSILON);
        assert!((pf(7, 1).confidence() - 7.0 / 9.0).abs() < f64::EPSILON);
        assert!((pf(0, 5).confidence() - 0.0).abs() < f64::EPSILON);
        assert!((pf(3, 0).confidence() - 3.0 / 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_frontmatter_from_string() {
        let content = "---\ndescription: Test skill\ntags:\n  - rust\nagent_authored: true\nhelpful: 3\nharmful: 1\n---\n# My Skill\n\nSome content here.";
        let (fm, body) = PromptFile::parse_frontmatter(content).unwrap();
        assert_eq!(fm.description, "Test skill");
        assert_eq!(fm.tags, vec!["rust"]);
        assert!(fm.agent_authored);
        assert_eq!(fm.helpful, 3);
        assert_eq!(fm.harmful, 1);
        assert!(body.contains("# My Skill"));
        assert!(body.contains("Some content here."));
    }

    #[test]
    fn write_and_parse_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let skill_path = temp_dir.path().join("my-skill").join(SKILL_FILENAME);

        let prompt = PromptFile {
            name: "my-skill".to_string(),
            description: "Test skill".to_string(),
            body: "# My Skill\n\nSome content here.".to_string(),
            path: skill_path.clone(),
            user_invocable: false,
            agent_invocable: true,
            argument_hint: None,
            tags: vec!["convention".to_string()],
            triggers: PromptTriggers::default(),
            agent_authored: true,
            helpful: 2,
            harmful: 1,
        };
        prompt.write(&skill_path).unwrap();

        let parsed = PromptFile::parse(&skill_path).unwrap();
        assert_eq!(parsed.description, "Test skill");
        assert_eq!(parsed.tags, vec!["convention"]);
        assert!(parsed.agent_authored);
        assert_eq!(parsed.helpful, 2);
        assert_eq!(parsed.harmful, 1);
        assert!(parsed.body.contains("# My Skill"));
        assert!(parsed.body.contains("Some content here."));
    }

    #[test]
    fn write_empty_body() {
        let temp_dir = TempDir::new().unwrap();
        let skill_path = temp_dir.path().join("empty-body").join(SKILL_FILENAME);

        let prompt = PromptFile {
            name: "empty-body".to_string(),
            description: "Empty".to_string(),
            body: String::new(),
            path: skill_path.clone(),
            user_invocable: false,
            agent_invocable: true,
            argument_hint: None,
            tags: vec![],
            triggers: PromptTriggers::default(),
            agent_authored: true,
            helpful: 0,
            harmful: 0,
        };
        prompt.write(&skill_path).unwrap();

        let raw = std::fs::read_to_string(&skill_path).unwrap();
        assert!(raw.starts_with("---\n"));
        assert!(raw.contains("description: Empty"));
    }

    #[test]
    fn write_and_parse_roundtrip_with_triggers() {
        let temp_dir = TempDir::new().unwrap();
        let skill_path = temp_dir.path().join("rust-rules").join(SKILL_FILENAME);

        let triggers = PromptTriggers::new(vec!["src/**/*.rs".to_string(), "tests/**/*.rs".to_string()]).unwrap();

        let prompt = PromptFile {
            name: "rust-rules".to_string(),
            description: "Rust conventions".to_string(),
            body: "Follow Rust conventions.".to_string(),
            path: skill_path.clone(),
            user_invocable: false,
            agent_invocable: false,
            argument_hint: None,
            tags: vec![],
            triggers,
            agent_authored: false,
            helpful: 0,
            harmful: 0,
        };
        prompt.write(&skill_path).unwrap();

        let parsed = PromptFile::parse(&skill_path).unwrap();
        assert_eq!(parsed.description, "Rust conventions");
        assert!(!parsed.triggers.is_empty());
        assert!(parsed.triggers.matches_read("src/main.rs"));
        assert!(parsed.triggers.matches_read("tests/integration.rs"));
        assert!(!parsed.triggers.matches_read("README.md"));
        assert_eq!(parsed.triggers.patterns(), &["src/**/*.rs", "tests/**/*.rs"]);
    }

    #[test]
    fn write_rejects_empty_description() {
        let temp_dir = TempDir::new().unwrap();
        let skill_path = temp_dir.path().join("bad").join(SKILL_FILENAME);

        let prompt = PromptFile {
            name: "bad".to_string(),
            description: String::new(),
            body: "content".to_string(),
            path: skill_path.clone(),
            user_invocable: true,
            agent_invocable: false,
            argument_hint: None,
            tags: vec![],
            triggers: PromptTriggers::default(),
            agent_authored: true,
            helpful: 0,
            harmful: 0,
        };
        let result = prompt.write(&skill_path);
        assert!(matches!(result, Err(PromptFileError::MissingDescription { .. })));
    }

    #[test]
    fn write_rejects_no_activation_surface() {
        let temp_dir = TempDir::new().unwrap();
        let skill_path = temp_dir.path().join("noop").join(SKILL_FILENAME);

        let prompt = PromptFile {
            name: "noop".to_string(),
            description: "Does nothing".to_string(),
            body: "content".to_string(),
            path: skill_path.clone(),
            user_invocable: false,
            agent_invocable: false,
            argument_hint: None,
            tags: vec![],
            triggers: PromptTriggers::default(),
            agent_authored: true,
            helpful: 0,
            harmful: 0,
        };
        let result = prompt.write(&skill_path);
        assert!(matches!(result, Err(PromptFileError::NoActivationSurface { .. })));
    }

    #[test]
    fn skip_serializing_defaults() {
        let fm = minimal_frontmatter("Minimal");

        let yaml = serde_yml::to_string(&fm).unwrap();
        assert!(!yaml.contains("tags"));
        assert!(!yaml.contains("agent_authored"));
        assert!(!yaml.contains("helpful"));
        assert!(!yaml.contains("harmful"));
    }
}
