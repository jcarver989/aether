use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::skills::skill_file::{SkillEntry, SkillFile, SkillFileError};

const PRUNE_CONFIDENCE_THRESHOLD: f64 = 0.2;
const PRUNE_MIN_EVALUATIONS: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScoreSkillEntryInput {
    /// Skill name (directory name)
    pub skill: String,
    /// Entry ID to score
    pub entry_id: String,
    /// true = helpful, false = harmful
    pub helpful: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScoreStatus {
    Scored,
    Pruned,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScoreSkillEntryOutput {
    pub entry_id: String,
    pub skill_name: String,
    pub status: ScoreStatus,
    pub confidence: f64,
    pub message: String,
}

pub fn score_skill_entry(
    input: &ScoreSkillEntryInput,
    skills_dir: &Path,
) -> Result<ScoreSkillEntryOutput, SkillFileError> {
    let dir = skills_dir.join(&input.skill);
    if !SkillFile::exists(&dir) {
        return Err(SkillFileError::NotFound(input.skill.clone()));
    }

    let mut skill_file = SkillFile::open(&dir)?;

    let entry = skill_file.find_entry_mut(&input.entry_id)?;
    if input.helpful {
        entry.helpful_count += 1;
    } else {
        entry.harmful_count += 1;
    }
    let confidence = entry.confidence();
    let (helpful, harmful) = (entry.helpful_count, entry.harmful_count);
    let total = helpful + harmful;
    let should_prune = confidence < PRUNE_CONFIDENCE_THRESHOLD && total >= PRUNE_MIN_EVALUATIONS;

    if should_prune {
        let pruned = skill_file.remove_entry(&input.entry_id).unwrap();
        archive_pruned_entry(skills_dir, &input.skill, &pruned)?;
        skill_file.save()?;

        Ok(ScoreSkillEntryOutput {
            entry_id: input.entry_id.clone(),
            skill_name: input.skill.clone(),
            status: ScoreStatus::Pruned,
            confidence,
            message: format!(
                "Entry '{}' pruned from '{}' (confidence: {confidence:.2}). Logged to .archived/.",
                input.entry_id, input.skill
            ),
        })
    } else {
        skill_file.save()?;

        let direction = if input.helpful { "helpful" } else { "harmful" };
        Ok(ScoreSkillEntryOutput {
            entry_id: input.entry_id.clone(),
            skill_name: input.skill.clone(),
            status: ScoreStatus::Scored,
            confidence,
            message: format!(
                "Entry '{}' marked as {direction}. Confidence: {confidence:.2} (+{helpful}/-{harmful})",
                input.entry_id
            ),
        })
    }
}

fn archive_pruned_entry(
    skills_dir: &Path,
    skill_name: &str,
    entry: &SkillEntry,
) -> Result<(), SkillFileError> {
    let archive_dir = skills_dir.join(".archived").join(skill_name);
    std::fs::create_dir_all(&archive_dir)?;

    let log_path = archive_dir.join("pruned.log");
    let log_entry = format!(
        "--- pruned entry {} (+{}/-{}) ---\n{}\n\n",
        entry.id, entry.helpful_count, entry.harmful_count, entry.content
    );

    let mut existing = std::fs::read_to_string(&log_path).unwrap_or_default();
    existing.push_str(&log_entry);
    std::fs::write(&log_path, existing)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::skill_file::SKILL_FILENAME;
    use tempfile::TempDir;

    fn create_skill_with_entries(skills_dir: &Path, name: &str, entries_text: &str) {
        let dir = skills_dir.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let content = format!(
            "---\ndescription: Test skill\n---\n# Test\n\nHuman content.\n\n## Agent Entries\n\n{entries_text}"
        );
        std::fs::write(dir.join(SKILL_FILENAME), content).unwrap();
    }

    #[test]
    fn test_score_helpful() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_skill_with_entries(skills_dir, "tips", "### abc123 (+0/-0)\nA tip.\n");

        let input = ScoreSkillEntryInput {
            skill: "tips".to_string(),
            entry_id: "abc123".to_string(),
            helpful: true,
        };

        let output = score_skill_entry(&input, skills_dir).unwrap();
        assert_eq!(output.status, ScoreStatus::Scored);
        assert!(output.confidence > 0.0);

        // Verify on disk
        let content = std::fs::read_to_string(skills_dir.join("tips/SKILL.md")).unwrap();
        assert!(content.contains("### abc123 (+1/-0)"));
    }

    #[test]
    fn test_score_harmful() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_skill_with_entries(skills_dir, "tips", "### abc123 (+0/-0)\nA tip.\n");

        let input = ScoreSkillEntryInput {
            skill: "tips".to_string(),
            entry_id: "abc123".to_string(),
            helpful: false,
        };

        let output = score_skill_entry(&input, skills_dir).unwrap();
        assert_eq!(output.status, ScoreStatus::Scored);
        assert_eq!(output.confidence, 0.0);

        let content = std::fs::read_to_string(skills_dir.join("tips/SKILL.md")).unwrap();
        assert!(content.contains("### abc123 (+0/-1)"));
    }

    #[test]
    fn test_auto_prune() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        // Start with 0 helpful and 2 harmful — one more harmful triggers prune
        // total = 3, confidence = 0/4 = 0.0 < 0.2
        create_skill_with_entries(skills_dir, "tips", "### abc123 (+0/-2)\nBad tip.\n");

        let input = ScoreSkillEntryInput {
            skill: "tips".to_string(),
            entry_id: "abc123".to_string(),
            helpful: false,
        };

        let output = score_skill_entry(&input, skills_dir).unwrap();
        assert_eq!(output.status, ScoreStatus::Pruned);

        // Entry should be removed from SKILL.md
        let content = std::fs::read_to_string(skills_dir.join("tips/SKILL.md")).unwrap();
        assert!(!content.contains("abc123"));

        // Should be in archive
        let archive =
            std::fs::read_to_string(skills_dir.join(".archived/tips/pruned.log")).unwrap();
        assert!(archive.contains("abc123"));
        assert!(archive.contains("Bad tip."));
    }

    #[test]
    fn test_prune_preserves_other_entries() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let entries = "### bad111 (+0/-2)\nBad tip.\n\n### a00d22 (+5/-0)\nGood tip.\n";
        create_skill_with_entries(skills_dir, "tips", entries);

        let input = ScoreSkillEntryInput {
            skill: "tips".to_string(),
            entry_id: "bad111".to_string(),
            helpful: false,
        };

        let output = score_skill_entry(&input, skills_dir).unwrap();
        assert_eq!(output.status, ScoreStatus::Pruned);

        let content = std::fs::read_to_string(skills_dir.join("tips/SKILL.md")).unwrap();
        assert!(!content.contains("bad111"));
        assert!(content.contains("a00d22"));
        assert!(content.contains("Good tip."));
    }

    #[test]
    fn test_score_nonexistent_skill() {
        let temp_dir = TempDir::new().unwrap();

        let input = ScoreSkillEntryInput {
            skill: "nonexistent".to_string(),
            entry_id: "abc123".to_string(),
            helpful: true,
        };

        let result = score_skill_entry(&input, temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_score_nonexistent_entry() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_skill_with_entries(skills_dir, "tips", "### abc123 (+0/-0)\nA tip.\n");

        let input = ScoreSkillEntryInput {
            skill: "tips".to_string(),
            entry_id: "xxxxxx".to_string(),
            helpful: true,
        };

        let result = score_skill_entry(&input, skills_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_prune_preserves_human_content() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        create_skill_with_entries(skills_dir, "tips", "### abc123 (+0/-2)\nBad tip.\n");

        let input = ScoreSkillEntryInput {
            skill: "tips".to_string(),
            entry_id: "abc123".to_string(),
            helpful: false,
        };

        score_skill_entry(&input, skills_dir).unwrap();

        let content = std::fs::read_to_string(skills_dir.join("tips/SKILL.md")).unwrap();
        assert!(content.contains("# Test"));
        assert!(content.contains("Human content."));
        // No agent entries section when all entries pruned
        assert!(!content.contains("## Agent Entries"));
    }
}
