use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use aether_project::{PromptFile, PromptFileError, SKILL_FILENAME};

const PRUNE_CONFIDENCE_THRESHOLD: f64 = 0.2;
const PRUNE_MIN_EVALUATIONS: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RateSkillInput {
    /// Skill name (directory name)
    pub name: String,
    /// true = helpful, false = harmful
    pub helpful: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RateSkillStatus {
    Scored,
    Pruned,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RateSkillOutput {
    pub name: String,
    pub status: RateSkillStatus,
    pub confidence: f64,
    pub message: String,
}

pub fn rate_skill(
    input: &RateSkillInput,
    skills_dir: &Path,
) -> Result<RateSkillOutput, PromptFileError> {
    let skill_path = skills_dir.join(&input.name).join(SKILL_FILENAME);
    if !skill_path.is_file() {
        return Err(PromptFileError::NotFound(input.name.clone()));
    }

    let mut prompt = PromptFile::parse(&skill_path)?;

    if !prompt.agent_authored {
        return Err(PromptFileError::NotAgentAuthored(input.name.clone()));
    }

    if input.helpful {
        prompt.helpful += 1;
    } else {
        prompt.harmful += 1;
    }

    let confidence = prompt.confidence();
    let total = prompt.helpful + prompt.harmful;
    let should_prune = confidence < PRUNE_CONFIDENCE_THRESHOLD && total >= PRUNE_MIN_EVALUATIONS;

    if should_prune {
        archive_pruned_skill(skills_dir, &input.name, &prompt)?;
        std::fs::remove_dir_all(skills_dir.join(&input.name))?;

        Ok(RateSkillOutput {
            name: input.name.clone(),
            status: RateSkillStatus::Pruned,
            confidence,
            message: format!(
                "Skill '{}' pruned (confidence: {confidence:.2}). Logged to .archived/.",
                input.name
            ),
        })
    } else {
        prompt.write(&skill_path)?;

        let direction = if input.helpful { "helpful" } else { "harmful" };
        Ok(RateSkillOutput {
            name: input.name.clone(),
            status: RateSkillStatus::Scored,
            confidence,
            message: format!(
                "Skill '{}' marked as {direction}. Confidence: {confidence:.2} (+{}/-{})",
                input.name, prompt.helpful, prompt.harmful
            ),
        })
    }
}

fn archive_pruned_skill(
    skills_dir: &Path,
    skill_name: &str,
    prompt: &PromptFile,
) -> Result<(), PromptFileError> {
    let archive_dir = skills_dir.join(".archived").join(skill_name);
    std::fs::create_dir_all(&archive_dir)?;

    let log_path = archive_dir.join("pruned.log");
    let log_entry = format!(
        "--- pruned skill '{}' (+{}/-{}, confidence: {:.2}) ---\n{}\n\n{}\n\n",
        skill_name,
        prompt.helpful,
        prompt.harmful,
        prompt.confidence(),
        prompt.description,
        prompt.body,
    );

    let mut existing = std::fs::read_to_string(&log_path).unwrap_or_default();
    existing.push_str(&log_entry);
    std::fs::write(&log_path, existing)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_project::SKILL_FILENAME;
    use std::fmt::Write;
    use tempfile::TempDir;

    fn create_agent_skill(skills_dir: &Path, name: &str, helpful: u32, harmful: u32) {
        let dir = skills_dir.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let mut content =
            "---\ndescription: Test skill\nagent-invocable: true\nagent_authored: true\n"
                .to_string();
        if helpful > 0 {
            writeln!(content, "helpful: {helpful}").expect("write to String should not fail");
        }
        if harmful > 0 {
            writeln!(content, "harmful: {harmful}").expect("write to String should not fail");
        }
        content.push_str("---\nSome content.\n");
        std::fs::write(dir.join(SKILL_FILENAME), content).unwrap();
    }

    #[test]
    fn test_rate_helpful() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();
        create_agent_skill(skills_dir, "tips", 0, 0);

        let input = RateSkillInput {
            name: "tips".to_string(),
            helpful: true,
        };

        let output = rate_skill(&input, skills_dir).unwrap();
        assert_eq!(output.status, RateSkillStatus::Scored);
        assert!(output.confidence > 0.0);

        let fm = PromptFile::parse(&skills_dir.join("tips").join(SKILL_FILENAME)).unwrap();
        assert_eq!(fm.helpful, 1);
        assert_eq!(fm.harmful, 0);
    }

    #[test]
    fn test_rate_harmful() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();
        create_agent_skill(skills_dir, "tips", 0, 0);

        let input = RateSkillInput {
            name: "tips".to_string(),
            helpful: false,
        };

        let output = rate_skill(&input, skills_dir).unwrap();
        assert_eq!(output.status, RateSkillStatus::Scored);
        assert!(output.confidence.abs() < f64::EPSILON);

        let fm = PromptFile::parse(&skills_dir.join("tips").join(SKILL_FILENAME)).unwrap();
        assert_eq!(fm.helpful, 0);
        assert_eq!(fm.harmful, 1);
    }

    #[test]
    fn test_auto_prune() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();
        // Start with 0 helpful, 2 harmful — one more triggers prune
        create_agent_skill(skills_dir, "bad-skill", 0, 2);

        let input = RateSkillInput {
            name: "bad-skill".to_string(),
            helpful: false,
        };

        let output = rate_skill(&input, skills_dir).unwrap();
        assert_eq!(output.status, RateSkillStatus::Pruned);

        // Skill directory should be removed
        assert!(!skills_dir.join("bad-skill").exists());

        // Should be in archive
        let archive =
            std::fs::read_to_string(skills_dir.join(".archived/bad-skill/pruned.log")).unwrap();
        assert!(archive.contains("bad-skill"));
        assert!(archive.contains("Some content."));
    }

    #[test]
    fn test_reject_human_skill() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let dir = skills_dir.join("human-skill");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(SKILL_FILENAME),
            "---\ndescription: Human skill\n---\n# Human content\n",
        )
        .unwrap();

        let input = RateSkillInput {
            name: "human-skill".to_string(),
            helpful: true,
        };
        let result = rate_skill(&input, skills_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonexistent_skill() {
        let temp_dir = TempDir::new().unwrap();

        let input = RateSkillInput {
            name: "nonexistent".to_string(),
            helpful: true,
        };
        let result = rate_skill(&input, temp_dir.path());
        assert!(result.is_err());
    }
}
