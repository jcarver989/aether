use rmcp::model::{Prompt, RawResource, Resource};
use serde::{Deserialize, Serialize};

use crate::MarkdownFile;

pub type PromptFile = MarkdownFile<PromptFrontmatter>;
pub type SkillsFile = MarkdownFile<SkillsFrontmatter>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFrontmatter {
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsFrontmatter {
    pub description: Option<String>,
}

impl From<&PromptFile> for Prompt {
    fn from(file: &PromptFile) -> Self {
        Prompt::new(
            file.name.clone(),
            file.frontmatter
                .as_ref()
                .and_then(|f| f.description.clone()),
            None,
        )
    }
}

impl From<&SkillsFile> for Resource {
    fn from(file: &SkillsFile) -> Self {
        let mut resource = RawResource::new(format!("skill://{}", &file.name), file.name.clone());
        resource.mime_type = Some("text/markdown".to_string());
        resource.description = file
            .frontmatter
            .as_ref()
            .and_then(|f| f.description.clone());

        Resource::new(resource, None)
    }
}
