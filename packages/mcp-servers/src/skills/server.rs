use clap::Parser;
use mcp_utils::substitution::substitute_parameters;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{
        GetPromptRequestParams, GetPromptResult, Implementation, ListPromptsResult,
        PaginatedRequestParams, PromptMessage, PromptMessageRole, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::error;

use super::tools::{
    LoadSkillsInput, LoadSkillsOutput, RateSkillInput, RateSkillOutput, SaveSkillInput,
    SaveSkillOutput, Skill, rate_skill, save_skill,
};
use crate::skills::skill_file::{SKILL_FILENAME, SkillMetadata, SkillsFile, load_skill_metadata};
use crate::skills::{
    prompt_file::{PromptFile, to_prompt},
    tools::rate_skill::RateSkillStatus,
};

/// CLI arguments for `SkillsMcp` server
#[derive(Debug, Clone, Parser)]
pub struct SkillsMcpArgs {
    /// Base directory for skills (contains 'commands' and 'skills' subdirectories)
    #[arg(long = "dir")]
    pub base_dir: Option<PathBuf>,
}

impl SkillsMcpArgs {
    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let mut full_args = vec!["skills-mcp".to_string()];
        full_args.extend(args);

        Self::try_parse_from(full_args)
            .map_err(|e| format!("Failed to parse SkillsMcp arguments: {e}"))
    }
}

/// MCP server for skills and slash-commands
#[derive(Clone)]
pub struct SkillsMcp {
    commands_dir: PathBuf,
    skills_dir: PathBuf,
    skills_info: Arc<RwLock<Vec<SkillMetadata>>>,
    tool_router: ToolRouter<Self>,
    roots: Arc<RwLock<Vec<PathBuf>>>,
}

impl SkillsMcp {
    pub fn new(base_dir: PathBuf) -> Self {
        let skills_dir = base_dir.join("skills");
        let skills_info = load_skill_metadata(&skills_dir);

        Self {
            commands_dir: base_dir.join("commands"),
            skills_dir,
            skills_info: Arc::new(RwLock::new(skills_info)),
            tool_router: Self::tool_router(),
            roots: Arc::new(RwLock::new(vec![base_dir])),
        }
    }

    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let parsed_args = SkillsMcpArgs::from_args(args)?;
        let base_dir = parsed_args.base_dir.unwrap_or_else(|| PathBuf::from("."));
        Ok(Self::new(base_dir))
    }

    pub fn with_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.roots = Arc::new(RwLock::new(roots));
        self
    }

    fn build_instructions(skills_info: &[SkillMetadata]) -> String {
        let mut instructions = include_str!("./instructions.md").to_string();

        if !skills_info.is_empty() {
            instructions.push_str("\n\n## Complete List of Available Skills\n");
            instructions.push_str("You have access to the following Skills:\n\n");

            for skill in skills_info {
                use std::fmt::Write as _;
                if skill.tags.is_empty() {
                    let _ = writeln!(instructions, "- **{}**: {}", skill.name, skill.description);
                } else {
                    let tags = skill.tags.join(", ");
                    let _ = writeln!(
                        instructions,
                        "- **{}** [{}]: {}",
                        skill.name, tags, skill.description
                    );
                }
            }
        }

        instructions
    }

    /// Reload skill metadata from disk.
    async fn reload_metadata(&self) {
        let metadata = load_skill_metadata(&self.skills_dir);
        *self.skills_info.write().await = metadata;
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SkillsMcp {
    fn get_info(&self) -> ServerInfo {
        // try_read() avoids blocking the synchronous get_info() callback.
        // On contention (only possible during a concurrent tool call), we fall back
        // to an empty skill list — this only affects the MCP handshake instructions,
        // and the tools themselves always read fresh data.
        let skills_info = self
            .skills_info
            .try_read()
            .map(|g| g.clone())
            .unwrap_or_default();
        ServerInfo {
            server_info: Implementation {
                name: "skills-mcp".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(Self::build_instructions(&skills_info)),
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        let command_files_with_paths = match PromptFile::from_dir(&self.commands_dir).await {
            Ok(files) => files,
            Err(e) => {
                error!(
                    "Failed to load prompt files from {:?}: {}",
                    self.commands_dir, e
                );

                return Ok(ListPromptsResult {
                    prompts: Vec::new(),
                    next_cursor: None,
                    meta: None,
                });
            }
        };

        let commands = command_files_with_paths
            .iter()
            .filter_map(|(path, file)| {
                let name = path.file_stem()?.to_string_lossy().to_string();
                Some(to_prompt(file, name))
            })
            .collect();

        Ok(ListPromptsResult {
            prompts: commands,
            next_cursor: None,
            meta: None,
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        let prompt_path = self.commands_dir.join(format!("{}.md", request.name));
        let command_file = PromptFile::from_file(&prompt_path).map_err(|e| {
            McpError::invalid_params(format!("Prompt '{}' not found: {}", request.name, e), None)
        })?;

        let arguments = request.arguments.as_ref().map(|json_map| {
            json_map
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect::<HashMap<String, String>>()
        });

        let content = substitute_parameters(&command_file.content, &arguments);
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

#[tool_router]
impl SkillsMcp {
    #[doc = include_str!("tools/get_skills/description.md")]
    #[tool]
    pub async fn get_skills(
        &self,
        request: Parameters<LoadSkillsInput>,
    ) -> Result<Json<LoadSkillsOutput>, String> {
        let Parameters(args) = request;

        let mut skills = Vec::new();
        for skill_name in args.skills {
            let skill_path = self.skills_dir.join(&skill_name).join(SKILL_FILENAME);

            if let Ok(skill_file) = SkillsFile::from_file(&skill_path) {
                skills.push(Skill {
                    name: skill_name,
                    content: skill_file.content,
                });
            }
        }

        Ok(Json(LoadSkillsOutput { skills }))
    }

    #[doc = include_str!("tools/save_skill/description.md")]
    #[tool]
    pub async fn save_skill(
        &self,
        request: Parameters<SaveSkillInput>,
    ) -> Result<Json<SaveSkillOutput>, String> {
        let Parameters(input) = request;
        let result = save_skill(&input, &self.skills_dir).map_err(|e| e.to_string())?;

        self.reload_metadata().await;

        Ok(Json(result))
    }

    #[doc = include_str!("tools/rate_skill/description.md")]
    #[tool]
    pub async fn rate_skill(
        &self,
        request: Parameters<RateSkillInput>,
    ) -> Result<Json<RateSkillOutput>, String> {
        let Parameters(input) = request;
        let result = rate_skill(&input, &self.skills_dir).map_err(|e| e.to_string())?;

        if matches!(result.status, RateSkillStatus::Pruned) {
            self.reload_metadata().await;
        }

        Ok(Json(result))
    }
}
