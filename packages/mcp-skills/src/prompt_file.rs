use mcp_utils::MarkdownFile;
use rmcp::model::Prompt;
use serde::{Deserialize, Serialize};

pub type PromptFile = MarkdownFile<PromptFrontmatter>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFrontmatter {
    pub description: Option<String>,
}

pub fn to_prompt(file: &PromptFile, name: impl Into<String>) -> Prompt {
    Prompt::new(
        name.into(),
        file.frontmatter
            .as_ref()
            .and_then(|f| f.description.clone()),
        None,
    )
}
