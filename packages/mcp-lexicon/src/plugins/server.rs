use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::{
        GetPromptRequestParam, GetPromptResult, Implementation, ListPromptsResult,
        ListResourcesResult, PaginatedRequestParam, PromptMessage, PromptMessageRole,
        ReadResourceRequestParam, ReadResourceResult, ResourceContents, ServerCapabilities,
        ServerInfo,
    },
    service::RequestContext,
};
use std::path::PathBuf;

use super::{
    files::{PromptFile, SkillsFile},
    substitute_parameters,
};

/// MCP server that dynamically loads prompts from markdown files
#[derive(Clone)]
pub struct PluginsMcp {
    commands_dir: PathBuf,
    skills_dir: PathBuf,
}

impl PluginsMcp {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            commands_dir: base_dir.join("commands"),
            skills_dir: base_dir.join("skills"),
        }
    }
}

impl ServerHandler for PluginsMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "plugins-mcp".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: None,
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        async move {
            let skills_files = SkillsFile::from_dir(&self.skills_dir).await.map_err(|e| {
                McpError::internal_error(format!("Failed to load skills: {}", e), None)
            })?;

            let skills = skills_files.iter().map(|s| s.into()).collect();

            Ok(ListResourcesResult {
                resources: skills,
                next_cursor: None,
            })
        }
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let skill_name = request.uri.strip_prefix("skill://").ok_or_else(|| {
            McpError::invalid_params(
                format!(
                    "Invalid URI format: {}. Expected 'skill://<name>'",
                    request.uri
                ),
                None,
            )
        })?;

        let skill_path = self.skills_dir.join(format!("{}.md", skill_name));
        let skill = SkillsFile::from_file(&skill_path).map_err(|e| {
            McpError::invalid_params(format!("Skill '{}' not found: {}", skill_name, e), None)
        })?;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(skill.content, request.uri)],
        })
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send {
        async move {
            let command_files = PromptFile::from_dir(&self.commands_dir)
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("Failed to load commands: {}", e), None)
                })?;

            let commands = command_files.iter().map(|p| p.into()).collect();
            Ok(ListPromptsResult {
                prompts: commands,
                next_cursor: None,
            })
        }
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, McpError>> + Send {
        async move {
            let prompt_path = self.commands_dir.join(format!("{}.md", request.name));
            let command_file = PromptFile::from_file(&prompt_path).map_err(|e| {
                McpError::invalid_params(
                    format!("Prompt '{}' not found: {}", request.name, e),
                    None,
                )
            })?;

            let content = substitute_parameters(&command_file.content, &request.arguments);
            let messages = vec![PromptMessage::new_text(PromptMessageRole::User, content)];

            Ok(GetPromptResult {
                description: command_file
                    .frontmatter
                    .as_ref()
                    .and_then(|f| f.description.clone()),
                messages,
            })
        }
    }
}
