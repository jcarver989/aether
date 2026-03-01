use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::skills::skill_file::{
    SkillFileError, SkillsFrontmatter, read_and_parse, skill_exists, write_skill,
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SaveSkillInput {
    /// Skill name (directory name). Created if it doesn't exist, updated if it does.
    pub name: String,
    /// Short description for the TOC
    pub description: String,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Full markdown content of the skill
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SaveSkillStatus {
    Created,
    Updated,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SaveSkillOutput {
    pub name: String,
    pub status: SaveSkillStatus,
}

pub fn save_skill(
    input: &SaveSkillInput,
    skills_dir: &Path,
) -> Result<SaveSkillOutput, SkillFileError> {
    let dir = skills_dir.join(&input.name);

    let (frontmatter, status) = if skill_exists(&dir) {
        let (existing, _body) = read_and_parse(&dir)?;

        if !existing.agent_authored {
            return Err(SkillFileError::NotAgentAuthored(input.name.clone()));
        }

        let updated = SkillsFrontmatter {
            description: input.description.clone(),
            tags: input.tags.clone(),
            agent_authored: true,
            helpful: existing.helpful,
            harmful: existing.harmful,
        };
        (updated, SaveSkillStatus::Updated)
    } else {
        let new = SkillsFrontmatter {
            description: input.description.clone(),
            tags: input.tags.clone(),
            agent_authored: true,
            helpful: 0,
            harmful: 0,
        };
        (new, SaveSkillStatus::Created)
    };

    write_skill(&dir, &frontmatter, &input.content)?;

    Ok(SaveSkillOutput {
        name: input.name.clone(),
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::skill_file::SKILL_FILENAME;
    use tempfile::TempDir;

    #[test]
    fn test_create_new_skill() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let input = SaveSkillInput {
            name: "fake-not-mock".to_string(),
            description: "Always use Fake prefix, never Mock".to_string(),
            tags: vec!["convention".to_string(), "testing".to_string()],
            content: "When writing test doubles, use the Fake prefix.".to_string(),
        };

        let output = save_skill(&input, skills_dir).unwrap();
        assert_eq!(output.name, "fake-not-mock");
        assert_eq!(output.status, SaveSkillStatus::Created);

        let content = std::fs::read_to_string(skills_dir.join("fake-not-mock/SKILL.md")).unwrap();
        assert!(content.contains("description: Always use Fake prefix, never Mock"));
        assert!(content.contains("agent_authored: true"));
        assert!(content.contains("convention"));
        assert!(content.contains("testing"));
        assert!(content.contains("When writing test doubles"));
    }

    #[test]
    fn test_update_existing_agent_skill() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        // Create initial skill
        let input = SaveSkillInput {
            name: "my-skill".to_string(),
            description: "Original".to_string(),
            tags: vec!["rust".to_string()],
            content: "Original content.".to_string(),
        };
        save_skill(&input, skills_dir).unwrap();

        // Simulate some scoring happened
        let dir = skills_dir.join("my-skill");
        let (mut fm, _body) = read_and_parse(&dir).unwrap();
        fm.helpful = 3;
        fm.harmful = 1;
        write_skill(&dir, &fm, "Original content.").unwrap();

        // Update
        let input2 = SaveSkillInput {
            name: "my-skill".to_string(),
            description: "Updated".to_string(),
            tags: vec!["rust".to_string(), "convention".to_string()],
            content: "Updated content.".to_string(),
        };
        let output = save_skill(&input2, skills_dir).unwrap();
        assert_eq!(output.status, SaveSkillStatus::Updated);

        // Verify counters preserved
        let (fm2, body) = read_and_parse(&dir).unwrap();
        assert_eq!(fm2.description, "Updated");
        assert_eq!(fm2.tags, vec!["rust", "convention"]);
        assert_eq!(fm2.helpful, 3);
        assert_eq!(fm2.harmful, 1);
        assert!(body.contains("Updated content."));
    }

    #[test]
    fn test_reject_human_skill() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        // Create a human-authored skill (agent_authored absent / false)
        let dir = skills_dir.join("human-skill");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(SKILL_FILENAME),
            "---\ndescription: Human skill\n---\n# Human content\n",
        )
        .unwrap();

        let input = SaveSkillInput {
            name: "human-skill".to_string(),
            description: "Trying to overwrite".to_string(),
            tags: vec![],
            content: "Should fail.".to_string(),
        };
        let result = save_skill(&input, skills_dir);
        assert!(result.is_err());
    }
}
