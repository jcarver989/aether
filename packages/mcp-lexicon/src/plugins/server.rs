use clap::Parser;
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
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::{
    files::{AgentFile, PromptFile, SkillsFile},
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
            .map_err(|e| format!("Failed to parse PluginsMcp arguments: {e}"))
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentsOutput {
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpawnAgentInput {
    /// Name of the agent to spawn (must exist in sub-agents directory)
    pub agent_name: String,
    /// Task/prompt for the agent to perform
    pub prompt: String,
    /// Optional model override (e.g., "anthropic:claude-3.5-sonnet")
    /// If not provided, uses model from agent's AGENT.md frontmatter
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpawnAgentOutput {
    /// The agent's final output
    pub output: String,
    /// Whether the agent completed successfully
    pub success: bool,
}

/// MCP server that dynamically loads slash-commands and skills from markdown files
#[derive(Clone)]
pub struct PluginsMcp {
    commands_dir: PathBuf,
    skills_dir: PathBuf,
    agents_dir: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl PluginsMcp {
    /// Create a new PluginsMcp server with the given base directory
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            commands_dir: base_dir.join("commands"),
            skills_dir: base_dir.join("skills"),
            agents_dir: base_dir.join("sub-agents"),
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

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        let command_files_with_paths =
            PromptFile::from_dir(&self.commands_dir)
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("Failed to load commands: {e}"), None)
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

    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
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

#[tool_router]
impl PluginsMcp {
    #[tool(
        description = "List all available skills with their names and descriptions.

Returns an array of skills, each with:
- name: identifier for the skill
- description: summary of what the skill does

Use this to discover available skills before loading their full content with get_skills."
    )]
    pub async fn list_skills(&self) -> Result<Json<ListSkillsOutput>, String> {
        let skills_with_dirs = SkillsFile::from_nested_dirs(&self.skills_dir, "SKILL.md")
            .await
            .map_err(|e| format!("Failed to load skills: {e}"))?;

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

    #[tool(
        description = "List all available sub-agents with their names and descriptions.

Returns an array of agents, each with:
- name: identifier for the agent
- description: summary of what the agent does

Use this to discover available agents before spawning them with spawn_agent."
    )]
    pub async fn list_agents(&self) -> Result<Json<ListAgentsOutput>, String> {
        let agents_with_dirs = AgentFile::from_nested_dirs(&self.agents_dir, "AGENT.md")
            .await
            .map_err(|e| format!("Failed to load agents: {e}"))?;

        let agents = agents_with_dirs
            .iter()
            .filter_map(|(dir, file)| {
                let name = dir.file_name()?.to_string_lossy().to_string();
                let description = file
                    .frontmatter
                    .as_ref()
                    .and_then(|f| f.description.clone())
                    .unwrap_or_default();
                Some(AgentInfo { name, description })
            })
            .collect();

        Ok(Json(ListAgentsOutput { agents }))
    }

    #[tool(
        description = "Spawn a sub-agent to perform a specific task.

Takes:
- agent_name: name of the agent from the sub-agents directory
- prompt: the task for the agent to perform
- model: optional model override (e.g., 'anthropic:claude-3.5-sonnet')

The agent will be spawned with its system prompt from AGENT.md and the provided task.
Progress updates will be streamed as the agent works.

Returns the agent's final output and success status."
    )]
    pub async fn spawn_agent(
        &self,
        request: Parameters<SpawnAgentInput>,
    ) -> Result<Json<SpawnAgentOutput>, String> {
        let Parameters(args) = request;

        // Load agent configuration
        let agent_dir = self.agents_dir.join(&args.agent_name);
        if !agent_dir.exists() {
            return Err(format!("Agent '{}' not found", args.agent_name));
        }

        let agent_file_path = agent_dir.join("AGENT.md");
        let agent_file = AgentFile::from_file(&agent_file_path)
            .map_err(|e| format!("Failed to load agent file: {e}"))?;

        // Determine which model to use
        let model = args
            .model
            .or_else(|| {
                agent_file
                    .frontmatter
                    .as_ref()
                    .and_then(|f| f.model.clone())
            })
            .ok_or_else(|| {
                format!(
                    "No model specified. Provide --model parameter or set 'model' in {}/AGENT.md frontmatter",
                    args.agent_name
                )
            })?;

        // Build command to spawn aether
        let mut cmd = Command::new("aether");
        cmd.arg("--model")
            .arg(&model)
            .arg("--system")
            .arg(&agent_file.content)
            .arg("--prompt")
            .arg(&args.prompt)
            .current_dir(&agent_dir) // Set working directory to agent dir (for mcp.json)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn the process
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn aether process: {e}"))?;

        // Capture stdout and stderr
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Failed to capture stderr".to_string())?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut output_buffer = String::new();

        // Read output line by line
        // Note: In a real implementation with MCP progress support, we would send
        // progress notifications here. For now, we just collect the output.
        loop {
            tokio::select! {
                Ok(Some(line)) = stdout_reader.next_line() => {
                    output_buffer.push_str(&line);
                    output_buffer.push('\n');
                    // TODO: Send progress notification with line
                }
                Ok(Some(line)) = stderr_reader.next_line() => {
                    output_buffer.push_str("[stderr] ");
                    output_buffer.push_str(&line);
                    output_buffer.push('\n');
                    // TODO: Send progress notification with line
                }
                else => break,
            }
        }

        // Wait for process to complete
        let status = child
            .wait()
            .await
            .map_err(|e| format!("Failed to wait for process: {e}"))?;

        Ok(Json(SpawnAgentOutput {
            output: output_buffer,
            success: status.success(),
        }))
    }
}
