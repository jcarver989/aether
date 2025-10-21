use futures::future::join_all;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::{
        GetPromptRequestParam, GetPromptResult, Implementation, ListPromptsResult,
        PaginatedRequestParam, PromptMessage, PromptMessageRole, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
};
use std::{fs, io, path::PathBuf};

use super::{PromptFile, substitute_parameters};

/// MCP server that dynamically loads prompts from markdown files
#[derive(Clone)]
pub struct SlashCommandMcp {
    /// Directory containing markdown files
    commands_dir: PathBuf,
}

impl SlashCommandMcp {
    pub fn new(prompts_dir: PathBuf) -> Self {
        Self {
            commands_dir: prompts_dir,
        }
    }

    async fn load_prompts(&self) -> Result<Vec<PromptFile>, io::Error> {
        if !self.commands_dir.exists() {
            tracing::warn!(
                "Prompts directory does not exist: {}",
                self.commands_dir.display()
            );
            return Ok(Vec::new());
        }

        let paths: Vec<_> = fs::read_dir(&self.commands_dir)?
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                (path.extension().and_then(|s| s.to_str()) == Some("md")).then_some(path)
            })
            .collect();

        let parse_tasks: Vec<_> = paths
            .into_iter()
            .map(|path| {
                tokio::spawn(async move {
                    let result = PromptFile::parse(&path);
                    (path, result)
                })
            })
            .collect();

        let results = join_all(parse_tasks).await;
        let prompts = results
            .into_iter()
            .filter_map(|result| match result {
                Ok((_, Ok(prompt_file))) => Some(prompt_file),
                Ok((path, Err(e))) => {
                    tracing::warn!("Failed to parse {}: {}", path.display(), e);
                    None
                }
                Err(_) => None,
            })
            .collect();

        Ok(prompts)
    }
}

impl ServerHandler for SlashCommandMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "slash-commands".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "A prompt server that dynamically loads prompts from markdown files".to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_prompts().build(),
            ..Default::default()
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send {
        async move {
            let prompt_files = self.load_prompts().await.map_err(|e| {
                McpError::internal_error(format!("Failed to load prompts: {}", e), None)
            })?;

            let prompts = prompt_files.iter().map(|p| p.into()).collect();
            Ok(ListPromptsResult {
                prompts,
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
            let prompt_files = self.load_prompts().await.map_err(|e| {
                McpError::internal_error(format!("Failed to load prompts: {}", e), None)
            })?;

            let prompt_file = prompt_files
                .into_iter()
                .find(|p| p.name == request.name)
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Prompt '{}' not found", request.name), None)
                })?;

            let content = substitute_parameters(&prompt_file.template, &request.arguments);
            let messages = vec![PromptMessage::new_text(PromptMessageRole::User, content)];

            Ok(GetPromptResult {
                description: prompt_file.frontmatter.description,
                messages,
            })
        }
    }
}
