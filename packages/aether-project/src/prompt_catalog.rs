//! Unified prompt catalog for discovering and validating `.aether/skills/*/SKILL.md` artifacts.
//!
//! A single `SKILL.md` artifact can serve as:
//! - A **slash command** (`user-invocable: true`)
//! - A **skill** (`agent-invocable: true`)
//! - A **rule** (`triggers.read` globs)
//! - Any combination of the above

use crate::error::SettingsError;
use crate::prompt_file::{PromptFile, SKILL_FILENAME};
use std::collections::HashSet;
use std::fs::read_dir;
use std::path::Path;

/// A catalog of prompt artifacts discovered from `.aether/skills/`.
#[derive(Debug, Clone)]
pub struct PromptCatalog {
    specs: Vec<PromptFile>,
}

impl PromptCatalog {
    /// Discover and validate all prompt artifacts under `skills_dir/*/SKILL.md`.
    ///
    /// `skills_dir` is typically `<project_root>/.aether/skills` or `<base_dir>/skills`.
    pub fn from_dir(skills_dir: &Path) -> Result<Self, SettingsError> {
        let prompts: Vec<PromptFile> = read_dir(skills_dir)
            .map_err(|e| SettingsError::IoError(e.to_string()))?
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.'))
            .filter(|e| e.path().join(SKILL_FILENAME).is_file())
            .filter_map(|e| match PromptFile::parse(&e.path().join(SKILL_FILENAME)) {
                Ok(spec) => Some(spec),
                Err(err) => {
                    tracing::warn!("Skipping invalid skill at {}: {err}", e.path().display());
                    None
                }
            })
            .collect();

        validate_catalog(&prompts)?;

        Ok(Self { specs: prompts })
    }

    /// Create an empty catalog.
    pub fn empty() -> Self {
        Self { specs: Vec::new() }
    }

    /// All prompt specs in catalog order.
    pub fn all(&self) -> &[PromptFile] {
        &self.specs
    }

    /// Iterate over user-invocable prompts (slash commands).
    pub fn slash_commands(&self) -> impl Iterator<Item = &PromptFile> {
        self.specs.iter().filter(|s| s.user_invocable)
    }

    /// Iterate over agent-invocable prompts (skills).
    pub fn skills(&self) -> impl Iterator<Item = &PromptFile> {
        self.specs.iter().filter(|s| s.agent_invocable)
    }

    /// Find all prompt specs whose read triggers match the given project-relative path.
    pub fn matching_rules(&self, relative_path: &str) -> Vec<&PromptFile> {
        self.specs.iter().filter(|s| s.triggers.matches_read(relative_path)).collect()
    }
}

fn validate_catalog(specs: &[PromptFile]) -> Result<(), SettingsError> {
    let mut seen_names = HashSet::new();
    for spec in specs {
        if !seen_names.insert(&spec.name) {
            return Err(SettingsError::DuplicatePromptName { name: spec.name.clone() });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_temp_project() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    fn write_skill(dir: &Path, name: &str, content: &str) {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join(SKILL_FILENAME), content).unwrap();
    }

    #[test]
    fn discover_empty_project() {
        let dir = create_temp_project();
        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        assert!(catalog.all().is_empty());
    }

    #[test]
    fn discover_user_only_prompt() {
        let dir = create_temp_project();
        write_skill(
            dir.path(),
            "commit",
            "---\ndescription: Generate commit messages\nuser-invocable: true\n---\nGenerate a commit message.",
        );

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        assert_eq!(catalog.all().len(), 1);

        let spec = &catalog.all()[0];
        assert_eq!(spec.name, "commit");
        assert!(spec.user_invocable);
        assert!(!spec.agent_invocable);
        assert!(spec.triggers.is_empty());
    }

    #[test]
    fn discover_agent_only_prompt() {
        let dir = create_temp_project();
        write_skill(
            dir.path(),
            "explain-code",
            "---\ndescription: Explain code\nagent-invocable: true\n---\nExplain the code.",
        );

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        assert_eq!(catalog.all().len(), 1);

        let spec = &catalog.all()[0];
        assert!(spec.agent_invocable);
        assert!(!spec.user_invocable);
    }

    #[test]
    fn discover_rule_only_prompt() {
        let dir = create_temp_project();
        write_skill(
            dir.path(),
            "rust-rules",
            "---\ndescription: Rust conventions\ntriggers:\n  read:\n    - \"packages/**/*.rs\"\n---\nFollow Rust conventions.",
        );

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        assert_eq!(catalog.all().len(), 1);

        let spec = &catalog.all()[0];
        assert!(!spec.user_invocable);
        assert!(!spec.agent_invocable);
        assert!(!spec.triggers.is_empty());
        assert!(spec.triggers.matches_read("packages/foo/bar.rs"));
        assert!(!spec.triggers.matches_read("other/file.py"));
    }

    #[test]
    fn discover_dual_use_prompt() {
        let dir = create_temp_project();
        write_skill(
            dir.path(),
            "explain",
            "---\ndescription: Explain code\nuser-invocable: true\nagent-invocable: true\nargument-hint: \"[path]\"\n---\nExplain with diagrams.",
        );

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        let spec = &catalog.all()[0];
        assert!(spec.user_invocable);
        assert!(spec.agent_invocable);
        assert_eq!(spec.argument_hint.as_deref(), Some("[path]"));

        let user: Vec<_> = catalog.slash_commands().collect();
        assert_eq!(user.len(), 1);
        let agent: Vec<_> = catalog.skills().collect();
        assert_eq!(agent.len(), 1);
    }

    #[test]
    fn reject_duplicate_names() {
        let dir = create_temp_project();
        write_skill(dir.path(), "foo", "---\ndescription: First\nuser-invocable: true\n---\nContent.");
        // Second skill with explicit name override to "foo"
        write_skill(dir.path(), "bar", "---\nname: foo\ndescription: Second\nuser-invocable: true\n---\nContent.");

        let result = PromptCatalog::from_dir(dir.path());
        assert!(matches!(result, Err(SettingsError::DuplicatePromptName { .. })));
    }

    #[test]
    fn reject_missing_description() {
        let dir = create_temp_project();
        write_skill(dir.path(), "bad", "---\ndescription: \"\"\nuser-invocable: true\n---\nContent.");

        let catalog = PromptCatalog::from_dir(dir.path());
        // Should be skipped with a warning (parsed OK but validation fails in parse_skill_file)
        assert!(catalog.unwrap().all().is_empty());
    }

    #[test]
    fn reject_no_activation_surface() {
        let dir = create_temp_project();
        write_skill(dir.path(), "noop", "---\ndescription: Does nothing\n---\nContent.");

        let catalog = PromptCatalog::from_dir(dir.path());
        // Should be skipped with a warning
        assert!(catalog.unwrap().all().is_empty());
    }

    #[test]
    fn name_defaults_to_directory_name() {
        let dir = create_temp_project();
        write_skill(dir.path(), "my-skill", "---\ndescription: My skill\nagent-invocable: true\n---\nContent.");

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        assert_eq!(catalog.all()[0].name, "my-skill");
    }

    #[test]
    fn name_from_frontmatter_overrides_directory() {
        let dir = create_temp_project();
        write_skill(
            dir.path(),
            "dir-name",
            "---\nname: custom-name\ndescription: Custom\nuser-invocable: true\n---\nContent.",
        );

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        assert_eq!(catalog.all()[0].name, "custom-name");
    }

    #[test]
    fn matching_read_rules_finds_matches() {
        let dir = create_temp_project();
        write_skill(
            dir.path(),
            "rust-rules",
            "---\ndescription: Rust rules\ntriggers:\n  read:\n    - \"src/**/*.rs\"\n---\nRust rules.",
        );
        write_skill(
            dir.path(),
            "ts-rules",
            "---\ndescription: TS rules\ntriggers:\n  read:\n    - \"src/**/*.ts\"\n---\nTS rules.",
        );
        write_skill(dir.path(), "commit", "---\ndescription: Commit\nuser-invocable: true\n---\nCommit.");

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        let matches = catalog.matching_rules("src/main.rs");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "rust-rules");

        let matches = catalog.matching_rules("src/app.ts");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "ts-rules");

        let matches = catalog.matching_rules("README.md");
        assert!(matches.is_empty());
    }

    #[test]
    fn pure_rule_not_in_user_or_agent_invocable() {
        let dir = create_temp_project();
        write_skill(
            dir.path(),
            "rule",
            "---\ndescription: A rule\ntriggers:\n  read:\n    - \"*.rs\"\n---\nRule content.",
        );

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        assert_eq!(catalog.all().len(), 1);
        assert_eq!(catalog.slash_commands().count(), 0);
        assert_eq!(catalog.skills().count(), 0);
    }

    #[test]
    fn skips_hidden_directories() {
        let dir = create_temp_project();
        write_skill(dir.path(), ".archived", "---\ndescription: Archived\nuser-invocable: true\n---\nOld.");
        write_skill(dir.path(), "visible", "---\ndescription: Visible\nuser-invocable: true\n---\nNew.");

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        assert_eq!(catalog.all().len(), 1);
        assert_eq!(catalog.all()[0].name, "visible");
    }

    #[test]
    fn preserves_tags_and_metadata() {
        let dir = create_temp_project();
        write_skill(
            dir.path(),
            "tagged",
            "---\ndescription: Tagged skill\nagent-invocable: true\ntags:\n  - rust\n  - testing\nagent_authored: true\nhelpful: 5\nharmful: 1\n---\nContent.",
        );

        let catalog = PromptCatalog::from_dir(dir.path()).unwrap();
        let spec = &catalog.all()[0];
        assert_eq!(spec.tags, vec!["rust", "testing"]);
        assert!(spec.agent_authored);
        assert_eq!(spec.helpful, 5);
        assert_eq!(spec.harmful, 1);
    }
}
