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
        PaginatedRequestParams, Prompt, PromptArgument, PromptMessage, PromptMessageRole,
        ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::tools::{
    LoadSkillsInput, LoadSkillsOutput, RateSkillInput, RateSkillOutput, SaveSkillInput,
    SaveSkillOutput, Skill, rate_skill, save_skill,
};
use crate::skills::tools::rate_skill::RateSkillStatus;
use aether_project::{PromptCatalog, PromptFile, SKILL_FILENAME};

/// CLI arguments for `SkillsMcp` server
#[derive(Debug, Clone, Parser)]
pub struct SkillsMcpArgs {
    /// Base directory for skills (contains 'skills' subdirectory)
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

/// MCP server for unified prompt artifacts (skills, slash commands, and rules).
#[derive(Clone)]
pub struct SkillsMcp {
    skills_dir: PathBuf,
    catalog: Arc<RwLock<PromptCatalog>>,
    tool_router: ToolRouter<Self>,
    roots: Arc<RwLock<Vec<PathBuf>>>,
}

impl SkillsMcp {
    pub fn new(base_dir: PathBuf) -> Self {
        let skills_dir = base_dir.join("skills");
        let catalog = PromptCatalog::from_dir(&skills_dir).unwrap_or_else(|e| {
            tracing::warn!(
                "Failed to load skill catalog from {}: {e}",
                skills_dir.display()
            );
            PromptCatalog::empty()
        });

        Self {
            skills_dir,
            catalog: Arc::new(RwLock::new(catalog)),
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

    fn build_instructions(catalog: &PromptCatalog) -> String {
        let mut instructions = include_str!("./instructions.md").to_string();

        let agent_skills: Vec<_> = catalog.skills().collect();

        if !agent_skills.is_empty() {
            instructions.push_str("\n\n## Complete List of Available Skills\n");
            instructions.push_str("You have access to the following Skills:\n\n");

            for skill in agent_skills {
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

    /// Reload the prompt catalog from disk.
    async fn reload_catalog(&self) {
        match PromptCatalog::from_dir(&self.skills_dir) {
            Ok(catalog) => *self.catalog.write().await = catalog,
            Err(e) => tracing::warn!("Failed to reload skill catalog: {e}"),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SkillsMcp {
    fn get_info(&self) -> ServerInfo {
        // try_read() avoids blocking the synchronous get_info() callback.
        // On contention (only possible during a concurrent tool call), we fall back
        // to an empty catalog — this only affects the MCP handshake instructions,
        // and the tools themselves always read fresh data.
        let instructions = match self.catalog.try_read() {
            Ok(catalog) => Self::build_instructions(&catalog),
            Err(_) => Self::build_instructions(&PromptCatalog::empty()),
        };
        ServerInfo {
            server_info: Implementation {
                name: "skills-mcp".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(instructions),
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
        let catalog = self.catalog.read().await;
        let prompts = catalog
            .slash_commands()
            .map(|s| {
                let arguments = s.argument_hint.as_ref().map(|hint| {
                    vec![PromptArgument {
                        name: "ARGUMENTS".to_string(),
                        title: None,
                        description: Some(hint.clone()),
                        required: Some(false),
                    }]
                });

                Prompt::new(s.name.clone(), Some(s.description.clone()), arguments)
            })
            .collect();

        Ok(ListPromptsResult {
            prompts,
            next_cursor: None,
            meta: None,
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        let catalog = self.catalog.read().await;
        let spec = catalog
            .slash_commands()
            .find(|s| s.name == request.name.as_str())
            .ok_or_else(|| {
                McpError::invalid_params(format!("Prompt '{}' not found", request.name), None)
            })?;

        let body = spec.body.clone();

        let arguments = request.arguments.as_ref().map(|json_map| {
            json_map
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect::<HashMap<String, String>>()
        });

        let content = substitute_parameters(&body, &arguments);
        let messages = vec![PromptMessage::new_text(PromptMessageRole::User, content)];

        Ok(GetPromptResult {
            description: Some(spec.description.clone()),
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
            if let Ok(prompt) = PromptFile::parse(&skill_path) {
                skills.push(Skill {
                    name: skill_name,
                    content: prompt.body,
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

        self.reload_catalog().await;

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
            self.reload_catalog().await;
        }

        Ok(Json(result))
    }
}
