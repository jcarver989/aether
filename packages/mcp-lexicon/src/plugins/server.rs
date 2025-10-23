use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{
        GetPromptRequestParam, GetPromptResult, Implementation, ListPromptsResult,
        PaginatedRequestParam, PromptMessage, PromptMessageRole, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use clap::Parser;

use super::{
    files::{PromptFile, SkillsFile},
    substitute_parameters,
};

/// CLI arguments for PluginsMcp server
#[derive(Debug, Clone, Parser)]
pub struct PluginsMcpArgs {
    /// Base directory for plugins (contains 'commands' and 'skills' subdirectories)
    #[arg(long = "dir")]
    pub base_dir: Option<PathBuf>,
}

impl PluginsMcpArgs {
    /// Parse args from a vector of strings
    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        // Prepend a dummy program name since clap expects it
        let mut full_args = vec!["plugins-mcp".to_string()];
        full_args.extend(args);

        Self::try_parse_from(full_args)
            .map_err(|e| format!("Failed to parse PluginsMcp arguments: {}", e))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoadSkillsInput {
    /// Array of skill names to load, e.g. "kungfu"
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoadSkillsOutput {
    pub skills: Vec<Skill>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListSkillsOutput {
    pub skills: Vec<SkillInfo>,
}

/// MCP server that dynamically loads slash-commands and skills from markdown files
#[derive(Clone)]
pub struct PluginsMcp {
    commands_dir: PathBuf,
    skills_dir: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl PluginsMcp {
    /// Create a new PluginsMcp server with the given base directory
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            commands_dir: base_dir.join("commands"),
            skills_dir: base_dir.join("skills"),
            tool_router: Self::tool_router(),
        }
    }

    /// Create a new PluginsMcp server from parsed CLI arguments
    /// If no --dir argument is provided, uses the current directory
    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let parsed_args = PluginsMcpArgs::from_args(args)?;
        let base_dir = parsed_args.base_dir.unwrap_or_else(|| PathBuf::from("."));
        Ok(Self::new(base_dir))
    }
}

#[tool_handler(router = self.tool_router)]
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
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send {
        async move {
            let command_files_with_paths =
                PromptFile::from_dir(&self.commands_dir)
                    .await
                    .map_err(|e| {
                        McpError::internal_error(format!("Failed to load commands: {}", e), None)
                    })?;

            let commands = command_files_with_paths
                .iter()
                .filter_map(|(path, file)| {
                    let name = path.file_stem()?.to_string_lossy().to_string();
                    Some(file.to_prompt(name))
                })
                .collect();
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

#[tool_router]
impl PluginsMcp {
    #[tool(description = "List all available skills with their names and descriptions.

Returns an array of skills, each with:
- name: identifier for the skill
- description: summary of what the skill does

Use this to discover available skills before loading their full content with get_skills.")]
    pub async fn list_skills(&self) -> Result<Json<ListSkillsOutput>, String> {
        let skills_with_dirs = SkillsFile::from_nested_dirs(&self.skills_dir, "SKILL.md")
            .await
            .map_err(|e| format!("Failed to load skills: {}", e))?;

        let skills = skills_with_dirs
            .iter()
            .filter_map(|(dir, file)| {
                let name = dir.file_name()?.to_string_lossy().to_string();
                let description = file
                    .frontmatter
                    .as_ref()
                    .and_then(|f| f.description.clone())
                    .unwrap_or_default();
                Some(SkillInfo { name, description })
            })
            .collect();

        Ok(Json(ListSkillsOutput { skills }))
    }

    #[tool(description = "Load the full content of one or more skills by name.

Takes an array of skill names and loads them into your context.
Skills that don't exist are silently skipped.

Use list_skills first to discover available skills.")]
    pub async fn get_skills(
        &self,
        request: Parameters<LoadSkillsInput>,
    ) -> Result<Json<LoadSkillsOutput>, String> {
        let Parameters(args) = request;

        let mut skills = Vec::new();
        for skill_name in args.skills {
            let skill_path = self.skills_dir.join(&skill_name).join("SKILL.md");

            if let Ok(skill_file) = SkillsFile::from_file(&skill_path) {
                skills.push(Skill {
                    name: skill_name,
                    content: skill_file.content,
                });
            }
        }

        Ok(Json(LoadSkillsOutput { skills }))
    }
}
