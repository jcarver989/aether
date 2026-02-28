use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::skills::skill_file::{SkillFile, SkillFileError, SkillsFrontmatter};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddSkillEntryInput {
    /// Skill name (directory name). Created if it doesn't exist.
    pub skill: String,
    /// Short description for skill listing (required when creating a new skill)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_description: Option<String>,
    /// Markdown content of the entry
    pub content: String,
    /// Replace existing entry by ID (upsert). Resets counters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AddEntryStatus {
    Created,
    Replaced,
    AddedToExisting,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddSkillEntryOutput {
    pub entry_id: String,
    pub skill_name: String,
    pub status: AddEntryStatus,
}

pub fn add_skill_entry(
    input: &AddSkillEntryInput,
    skills_dir: &Path,
) -> Result<AddSkillEntryOutput, SkillFileError> {
    let dir = skills_dir.join(&input.skill);

    let (mut skill_file, is_new) = if SkillFile::exists(&dir) {
        (SkillFile::open(&dir)?, false)
    } else {
        let desc = input
            .skill_description
            .clone()
            .unwrap_or_else(|| input.skill.replace('-', " "));
        (
            SkillFile::create(&dir, SkillsFrontmatter { description: desc }),
            true,
        )
    };

    if let Some(desc) = &input.skill_description {
        skill_file.frontmatter.description.clone_from(desc);
    }

    let (entry_id, status) = if let Some(id) = &input.replace_id {
        let entry = skill_file.find_entry_mut(id)?;
        entry.content.clone_from(&input.content);
        entry.helpful_count = 0;
        entry.harmful_count = 0;
        (id.clone(), AddEntryStatus::Replaced)
    } else {
        let id = skill_file.add_entry(input.content.clone());
        let status = if is_new {
            AddEntryStatus::Created
        } else {
            AddEntryStatus::AddedToExisting
        };
        (id, status)
    };

    skill_file.save()?;

    Ok(AddSkillEntryOutput {
        entry_id,
        skill_name: input.skill.clone(),
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_new_skill_with_entry() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let input = AddSkillEntryInput {
            skill: "rust-tips".to_string(),
            skill_description: Some("Rust best practices".to_string()),
            content: "Use clone() in tests for readability.".to_string(),
            replace_id: None,
        };

        let output = add_skill_entry(&input, skills_dir).unwrap();
        assert_eq!(output.skill_name, "rust-tips");
        assert_eq!(output.status, AddEntryStatus::Created);
        assert_eq!(output.entry_id.len(), 6);

        // Verify on disk
        let content = std::fs::read_to_string(skills_dir.join("rust-tips/SKILL.md")).unwrap();
        assert!(content.contains("description: Rust best practices"));
        assert!(content.contains("## Agent Entries"));
        assert!(content.contains("Use clone() in tests"));
    }

    #[test]
    fn test_add_entry_to_existing_human_skill() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        // Create a human-authored skill (no agent entries section)
        let skill_dir = skills_dir.join("rust-basics");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\ndescription: Rust basics\n---\n# Rust Basics\n\nHuman-written content here.\n",
        )
        .unwrap();

        let input = AddSkillEntryInput {
            skill: "rust-basics".to_string(),
            skill_description: None,
            content: "Agent-discovered tip.".to_string(),
            replace_id: None,
        };

        let output = add_skill_entry(&input, skills_dir).unwrap();
        assert_eq!(output.status, AddEntryStatus::AddedToExisting);

        let content = std::fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        assert!(content.contains("# Rust Basics"));
        assert!(content.contains("Human-written content here."));
        assert!(content.contains("## Agent Entries"));
        assert!(content.contains("Agent-discovered tip."));
    }

    #[test]
    fn test_add_second_entry() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let input1 = AddSkillEntryInput {
            skill: "tips".to_string(),
            skill_description: Some("Tips".to_string()),
            content: "First tip.".to_string(),
            replace_id: None,
        };
        let out1 = add_skill_entry(&input1, skills_dir).unwrap();

        let input2 = AddSkillEntryInput {
            skill: "tips".to_string(),
            skill_description: None,
            content: "Second tip.".to_string(),
            replace_id: None,
        };
        let out2 = add_skill_entry(&input2, skills_dir).unwrap();

        assert_ne!(out1.entry_id, out2.entry_id);
        assert_eq!(out2.status, AddEntryStatus::AddedToExisting);

        let content = std::fs::read_to_string(skills_dir.join("tips/SKILL.md")).unwrap();
        assert!(content.contains("First tip."));
        assert!(content.contains("Second tip."));
    }

    #[test]
    fn test_replace_entry() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let input = AddSkillEntryInput {
            skill: "tips".to_string(),
            skill_description: Some("Tips".to_string()),
            content: "Original content.".to_string(),
            replace_id: None,
        };
        let out = add_skill_entry(&input, skills_dir).unwrap();
        let entry_id = out.entry_id;

        let replace_input = AddSkillEntryInput {
            skill: "tips".to_string(),
            skill_description: None,
            content: "Updated content.".to_string(),
            replace_id: Some(entry_id.clone()),
        };
        let out2 = add_skill_entry(&replace_input, skills_dir).unwrap();

        assert_eq!(out2.entry_id, entry_id);
        assert_eq!(out2.status, AddEntryStatus::Replaced);

        let content = std::fs::read_to_string(skills_dir.join("tips/SKILL.md")).unwrap();
        assert!(!content.contains("Original content."));
        assert!(content.contains("Updated content."));
        // Counters should be reset to +0/-0
        assert!(content.contains(&format!("### {entry_id} (+0/-0)")));
    }

    #[test]
    fn test_replace_nonexistent_entry() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        // Create skill with an entry first
        let input = AddSkillEntryInput {
            skill: "tips".to_string(),
            skill_description: Some("Tips".to_string()),
            content: "A tip.".to_string(),
            replace_id: None,
        };
        add_skill_entry(&input, skills_dir).unwrap();

        let replace_input = AddSkillEntryInput {
            skill: "tips".to_string(),
            skill_description: None,
            content: "Won't work.".to_string(),
            replace_id: Some("nonexistent".to_string()),
        };
        let result = add_skill_entry(&replace_input, skills_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_description_from_name() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let input = AddSkillEntryInput {
            skill: "my-cool-skill".to_string(),
            skill_description: None,
            content: "Some content.".to_string(),
            replace_id: None,
        };
        add_skill_entry(&input, skills_dir).unwrap();

        let content = std::fs::read_to_string(skills_dir.join("my-cool-skill/SKILL.md")).unwrap();
        assert!(content.contains("description: my cool skill"));
    }
}
