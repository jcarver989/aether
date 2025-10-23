use rmcp::model::Prompt;
use serde::{Deserialize, Serialize};

use crate::MarkdownFile;

pub type PromptFile = MarkdownFile<PromptFrontmatter>;
pub type SkillsFile = MarkdownFile<SkillsFrontmatter>;
pub type AgentFile = MarkdownFile<AgentFrontmatter>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFrontmatter {
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsFrontmatter {
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    pub description: String,
    pub model: String,
}

impl PromptFile {
    pub fn to_prompt(&self, name: impl Into<String>) -> Prompt {
        Prompt::new(
            name.into(),
            self.frontmatter
                .as_ref()
                .and_then(|f| f.description.clone()),
            None,
        )
    }
}
