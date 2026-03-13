use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use aether_project::{PromptFile, PromptFileError, PromptTriggers, SKILL_FILENAME};

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
#[serde(rename_all = "camelCase")]
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
) -> Result<SaveSkillOutput, PromptFileError> {
    let skill_path = skills_dir.join(&input.name).join(SKILL_FILENAME);

    let (prompt, status) = if skill_path.is_file() {
        let existing = PromptFile::parse(&skill_path)?;

        if !existing.agent_authored {
            return Err(PromptFileError::NotAgentAuthored(input.name.clone()));
        }

        let mut updated = existing;
        updated.description = input.description.clone();
        updated.agent_invocable = true;
        updated.tags = input.tags.clone();
        updated.agent_authored = true;
        updated.body = input.content.clone();
        (updated, SaveSkillStatus::Updated)
    } else {
        let new = PromptFile {
            name: input.name.clone(),
            description: input.description.clone(),
            body: input.content.clone(),
            path: skill_path.clone(),
            user_invocable: false,
            agent_invocable: true,
            argument_hint: None,
            tags: input.tags.clone(),
            triggers: PromptTriggers::default(),
            agent_authored: true,
            helpful: 0,
            harmful: 0,
        };
        (new, SaveSkillStatus::Created)
    };

    prompt.write(&skill_path)?;

    Ok(SaveSkillOutput {
        name: input.name.clone(),
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_project::SKILL_FILENAME;
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
        let skill_path = skills_dir.join("my-skill").join(SKILL_FILENAME);
        let mut parsed = PromptFile::parse(&skill_path).unwrap();
        parsed.helpful = 3;
        parsed.harmful = 1;
        parsed.write(&skill_path).unwrap();

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
        let updated = PromptFile::parse(&skill_path).unwrap();
        assert_eq!(updated.description, "Updated");
        assert_eq!(updated.tags, vec!["rust", "convention"]);
        assert_eq!(updated.helpful, 3);
        assert_eq!(updated.harmful, 1);
        assert!(updated.body.contains("Updated content."));
    }

    #[test]
    fn test_reject_empty_description() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let input = SaveSkillInput {
            name: "bad-skill".to_string(),
            description: String::new(),
            tags: vec![],
            content: "Some content.".to_string(),
        };

        let result = save_skill(&input, skills_dir);
        assert!(result.is_err());
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
