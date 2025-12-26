use aether::{
    agent::{AgentHandle, AgentMessage, UserMessage, agent, substitute_parameters},
    llm::{StreamingModelProvider, ToolDefinition, parser::ModelProviderParser},
    mcp::{mcp, run_mcp_task::McpCommand},
};
use clap::Parser;
use futures::FutureExt;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{
        GetPromptRequestParam, GetPromptResult, Implementation, ListPromptsResult,
        PaginatedRequestParam, ProgressNotificationParam, PromptMessage, PromptMessageRole,
        ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::error;

use super::files::{AgentFile, PromptFile, SkillsFile};
use crate::coding::CodingMcp;

/// CLI arguments for PluginsMcp server
#[derive(Debug, Clone, Parser)]
pub struct PluginsMcpArgs {
    /// Base directory for plugins (contains 'commands' and 'skills' subdirectories)
    #[arg(long = "dir")]
    pub base_dir: Option<PathBuf>,
}

impl PluginsMcpArgs {
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
pub struct SubAgentListItem {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentsOutput {
    pub agents: Vec<SubAgentListItem>,
}

/// Reference to a file artifact discovered or modified by a sub-agent
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactReference {
    /// Absolute file path
    pub path: String,
    /// How this file relates to the task: read, modified, discovered, relevant
    pub relation: String,
    /// Brief note on why this file matters
    pub note: Option<String>,
}

/// Structured output from a sub-agent, designed to prevent information loss
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructuredAgentOutput {
    /// Brief summary of what the agent accomplished
    pub summary: String,
    /// Files read, modified, or discovered as relevant
    pub artifacts: Vec<ArtifactReference>,
    /// Key decisions, findings, or conclusions
    pub decisions: Vec<String>,
    /// Recommended follow-up tasks
    pub next_steps: Vec<String>,
    /// Full detailed output (optional, for deep dives)
    pub details: Option<String>,
}

/// Input for a single agent task within a batch
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentTask {
    /// Name of the agent to spawn (must exist in sub-agents directory)
    pub agent_name: String,
    /// Task for the agent to perform
    pub prompt: String,
    /// Optional model override in the format of "provider:model" (e.g., "anthropic:claude-3.5-sonnet")
    pub model: Option<String>,
    /// Optional unique identifier for this task (auto-generated if not provided)
    pub task_id: Option<String>,
}

/// Input for spawning sub-agents
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpawnSubAgentsInput {
    /// Array of agent tasks to execute in parallel
    pub tasks: Vec<SubAgentTask>,
}

/// Status of a sub-agent execution
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SubAgentStatus {
    Success,
    Error,
    Cancelled,
}

/// Result from a single sub-agent
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentResult {
    /// Unique identifier for this task
    pub task_id: String,
    /// Name of the agent that executed
    pub agent_name: String,
    /// Status of execution
    pub status: SubAgentStatus,
    /// Structured output (present on success)
    pub output: Option<StructuredAgentOutput>,
    /// Error message if status is Error
    pub error: Option<String>,
}

/// Output from spawning sub-agents
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpawnSubAgentsOutput {
    /// Results from all spawned agents (order matches input tasks)
    pub results: Vec<SubAgentResult>,
    /// Number of successful completions
    pub success_count: usize,
    /// Number of failures
    pub error_count: usize,
}

/// Prompt instructions appended to sub-agent prompts to ensure structured output
const STRUCTURED_OUTPUT_INSTRUCTIONS: &str = r#"

## Required Output Format

You MUST respond with valid JSON matching this exact structure:

```json
{
  "summary": "Brief summary of what you accomplished",
  "artifacts": [
    {"path": "/absolute/path/to/file.rs", "relation": "read|modified|discovered", "note": "why relevant"}
  ],
  "decisions": ["Key decision or finding 1", "Key decision or finding 2"],
  "nextSteps": ["Recommended follow-up 1", "Recommended follow-up 2"],
  "details": "Optional detailed output if needed"
}
```

CRITICAL:
- Include ALL file paths you examined or referenced (do not summarize these away)
- Use absolute paths, not relative
- Be explicit about decisions and reasoning
- Only return the JSON, no other text
"#;

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
                Some(file.to_prompt(name))
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
        request: GetPromptRequestParam,
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
impl PluginsMcp {
    #[tool(
        description = "List all available skills with their names and descriptions.

Returns an array of skills, each with:
- name: identifier for the skill
- description: summary of what the skill does

Use this to discover available skills before loading their full content with get_skills."
    )]
    pub async fn list_skills(&self) -> Result<Json<ListSkillsOutput>, String> {
        let skills_with_dirs =
            match SkillsFile::from_nested_dirs(&self.skills_dir, "SKILL.md").await {
                Ok(skills) => skills,
                Err(e) => {
                    error!(
                        "Failed to load skill files from {:?}: {}",
                        self.skills_dir, e
                    );
                    return Ok(Json(ListSkillsOutput { skills: Vec::new() }));
                }
            };

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
    pub async fn list_subagents(&self) -> Result<Json<ListAgentsOutput>, String> {
        let agents_with_dirs =
            match AgentFile::from_nested_dirs(&self.agents_dir, "AGENTS.md").await {
                Ok(agents) => agents,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // If agents directory doesn't exist, return empty list
                    Vec::new()
                }
                Err(e) => return Err(format!("Failed to load agents: {e}")),
            };

        let agents = agents_with_dirs
            .iter()
            .filter_map(|(dir, file)| {
                let name = dir.file_name()?.to_string_lossy().to_string();
                let description = file
                    .frontmatter
                    .as_ref()
                    .map(|f| f.description.clone())
                    .unwrap_or_default();
                Some(SubAgentListItem { name, description })
            })
            .collect();

        Ok(Json(ListAgentsOutput { agents }))
    }

    #[tool(
        description = "Spawn sub-agents in parallel to perform tasks concurrently.

Takes an array of tasks, each with:
- agent_name: name of the agent from sub-agents directory
- prompt: the task for the agent to perform
- model: optional model override (e.g., 'anthropic:claude-3.5-sonnet')
- task_id: optional identifier (auto-generated if omitted)

All agents execute in parallel. Results are returned when ALL agents complete.
Each agent returns structured output with: summary, artifacts, decisions, next_steps.

Ideal for:
- Parallel codebase exploration
- Concurrent file analysis
- Multi-aspect code review"
    )]
    pub async fn spawn_subagent(
        &self,
        request: Parameters<SpawnSubAgentsInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<SpawnSubAgentsOutput>, String> {
        let Parameters(args) = request;

        // Assign task IDs to tasks that don't have them
        let tasks: Vec<SubAgentTask> = args
            .tasks
            .into_iter()
            .enumerate()
            .map(|(i, mut task)| {
                if task.task_id.is_none() {
                    task.task_id = Some(format!("task_{}", i));
                }
                task
            })
            .collect();

        // Early return if no tasks
        if tasks.is_empty() {
            return Ok(Json(SpawnSubAgentsOutput {
                results: vec![],
                success_count: 0,
                error_count: 0,
            }));
        }

        // Clone self and context fields needed for the spawned tasks
        let agents_dir = Arc::new(self.agents_dir.clone());
        let progress_token = context.meta.get_progress_token();
        let peer = Arc::new(context.peer.clone());

        // Spawn all agents in parallel
        let handles: Vec<_> = tasks
            .into_iter()
            .map(|task| {
                let agents_dir = Arc::clone(&agents_dir);
                let progress_token = progress_token.clone();
                let peer = Arc::clone(&peer);
                let this = self.clone();

                tokio::spawn(async move {
                    this.execute_single_agent(task, agents_dir, progress_token, peer)
                        .await
                })
            })
            .collect();

        // Wait for all agents to complete
        let results: Vec<SubAgentResult> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|join_result| {
                join_result.unwrap_or_else(|e| SubAgentResult {
                    task_id: "unknown".to_string(),
                    agent_name: "unknown".to_string(),
                    status: SubAgentStatus::Error,
                    output: None,
                    error: Some(format!("Task panicked: {}", e)),
                })
            })
            .collect();

        let success_count = results
            .iter()
            .filter(|r| matches!(r.status, SubAgentStatus::Success))
            .count();
        let error_count = results
            .iter()
            .filter(|r| matches!(r.status, SubAgentStatus::Error))
            .count();

        Ok(Json(SpawnSubAgentsOutput {
            results,
            success_count,
            error_count,
        }))
    }

    async fn create_llm(
        &self,
        agent_name: &str,
        agent_file: &AgentFile,
        model_override: Option<String>,
    ) -> Result<Box<dyn StreamingModelProvider>, String> {
        let model_spec = model_override
            .or_else(|| {
                agent_file
                    .frontmatter
                    .as_ref()
                    .map(|f| f.model.clone())
            })
            .ok_or_else(|| {
                format!(
                    "No model specified. Provide model parameter or set 'model' in {}/AGENTS.md frontmatter",
                    agent_name
                )
            })?;

        ModelProviderParser::default()
            .parse(&model_spec)
            .map_err(|e| format!("Failed to parse model spec '{}': {}", model_spec, e))
    }

    async fn spawn_mcps(
        &self,
        agent_dir: &Path,
    ) -> Result<
        (
            Vec<ToolDefinition>,
            mpsc::Sender<McpCommand>,
            JoinHandle<()>,
        ),
        String,
    > {
        let mcp_config_path = agent_dir.join("mcp.json");

        mcp()
            .register_in_memory_server(
                "coding",
                Box::new(|_args| async move { CodingMcp::new().into_dyn() }.boxed()),
            )
            .from_json_file(mcp_config_path.to_str().unwrap_or(""))
            .await
            .map_err(|e| format!("Failed to load mcp.json: {}", e))?
            .spawn()
            .await
            .map_err(|e| format!("Failed to spawn MCP manager: {}", e))
    }

    async fn spawn_aether_agent(
        &self,
        llm: Box<dyn StreamingModelProvider>,
        system_prompt: &str,
        mcp_tx: mpsc::Sender<McpCommand>,
        tools: Vec<ToolDefinition>,
    ) -> Result<
        (
            mpsc::Sender<UserMessage>,
            mpsc::Receiver<AgentMessage>,
            AgentHandle,
        ),
        String,
    > {
        let (user_tx, agent_rx, agent_handle) = agent(llm)
            .system(system_prompt)
            .tools(mcp_tx, tools)
            .spawn()
            .await
            .map_err(|e| format!("Failed to spawn agent: {}", e))?;

        Ok((user_tx, agent_rx, agent_handle))
    }

    /// Execute a single sub-agent and return its result
    async fn execute_single_agent(
        &self,
        task: SubAgentTask,
        agents_dir: Arc<PathBuf>,
        progress_token: Option<rmcp::model::ProgressToken>,
        peer: Arc<rmcp::service::Peer<RoleServer>>,
    ) -> SubAgentResult {
        let task_id = task
            .task_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let agent_name = task.agent_name.clone();

        match self
            .execute_single_agent_inner(task, agents_dir, progress_token, peer)
            .await
        {
            Ok(output) => SubAgentResult {
                task_id,
                agent_name,
                status: SubAgentStatus::Success,
                output: Some(output),
                error: None,
            },
            Err(error) => SubAgentResult {
                task_id,
                agent_name,
                status: SubAgentStatus::Error,
                output: None,
                error: Some(error),
            },
        }
    }

    /// Inner implementation of single agent execution that can return errors
    async fn execute_single_agent_inner(
        &self,
        task: SubAgentTask,
        agents_dir: Arc<PathBuf>,
        progress_token: Option<rmcp::model::ProgressToken>,
        peer: Arc<rmcp::service::Peer<RoleServer>>,
    ) -> Result<StructuredAgentOutput, String> {
        let task_id = task
            .task_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let agent_name = task.agent_name.clone();

        let agent_dir = agents_dir.join(&task.agent_name);
        if !agent_dir.exists() {
            return Err(format!("Agent '{}' not found", task.agent_name));
        }

        let agent_file_path = agent_dir.join("AGENTS.md");
        let agent_file = AgentFile::from_file(&agent_file_path)
            .map_err(|e| format!("Failed to load agent file: {e}"))?;

        let llm = self
            .create_llm(&task.agent_name, &agent_file, task.model)
            .await?;

        // Append structured output instructions to the system prompt
        let system_prompt = format!("{}{}", agent_file.content, STRUCTURED_OUTPUT_INSTRUCTIONS);

        let (tools, mcp_tx, _mcp_handle) = self.spawn_mcps(&agent_dir).await?;
        let (user_tx, mut agent_rx, _agent_handle) = self
            .spawn_aether_agent(llm, &system_prompt, mcp_tx, tools)
            .await?;

        user_tx
            .send(UserMessage::text(&task.prompt))
            .await
            .map_err(|e| format!("Failed to send message to agent: {}", e))?;

        let mut final_output = String::new();
        let mut message_counter = 0u64;
        let mut was_cancelled = false;
        let mut error_message: Option<String> = None;

        while let Some(message) = agent_rx.recv().await {
            message_counter += 1;

            // Send progress notification with task_id included
            if let Some(ref token) = progress_token {
                let progress_data = serde_json::json!({
                    "task_id": task_id,
                    "agent_name": agent_name,
                    "event": serde_json::to_value(&message).unwrap_or(serde_json::Value::Null),
                });

                let _ = peer
                    .notify_progress(ProgressNotificationParam {
                        progress_token: token.clone(),
                        progress: message_counter as f64,
                        total: None,
                        message: Some(progress_data.to_string()),
                    })
                    .await;
            }

            match &message {
                AgentMessage::Text {
                    chunk, is_complete, ..
                } => {
                    if *is_complete {
                        final_output = chunk.clone();
                    }
                }

                AgentMessage::Error { message } => {
                    error_message = Some(message.clone());
                    break;
                }

                AgentMessage::Cancelled { message } => {
                    was_cancelled = true;
                    error_message = Some(message.clone());
                    break;
                }

                AgentMessage::Done => {
                    break;
                }

                _ => {
                    // All other message types (ToolCall, ToolProgress, ToolResult, ToolError)
                    // are already forwarded via progress notification above
                }
            }
        }

        if was_cancelled {
            return Err(format!(
                "Agent cancelled: {}",
                error_message.unwrap_or_default()
            ));
        }

        if let Some(err) = error_message {
            return Err(format!("Agent error: {}", err));
        }

        // Parse the structured output
        parse_agent_output(&final_output)
    }
}

/// Parse agent output into structured format with fallback for non-JSON responses
fn parse_agent_output(raw_output: &str) -> Result<StructuredAgentOutput, String> {
    // Try to parse as JSON directly
    if let Ok(parsed) = serde_json::from_str::<StructuredAgentOutput>(raw_output) {
        return Ok(parsed);
    }

    // Try to extract JSON from markdown code block
    if let Some(json_str) = extract_json_from_markdown(raw_output)
        && let Ok(parsed) = serde_json::from_str::<StructuredAgentOutput>(&json_str)
    {
        return Ok(parsed);
    }

    // Fallback: wrap raw output in structured format
    Ok(StructuredAgentOutput {
        summary: "Agent did not return structured output".to_string(),
        artifacts: vec![],
        decisions: vec![],
        next_steps: vec![],
        details: Some(raw_output.to_string()),
    })
}

/// Extract JSON from markdown code blocks (```json ... ``` or ``` ... ```)
fn extract_json_from_markdown(text: &str) -> Option<String> {
    // Look for ```json ... ``` pattern
    let json_block_start = text.find("```json")?;
    let content_start = json_block_start + 7;
    let remaining = &text[content_start..];

    // Find the closing ```
    let content_end = remaining.find("```")?;
    let json_content = remaining[..content_end].trim();

    if json_content.is_empty() {
        return None;
    }

    Some(json_content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_output_valid_json() {
        let json = r#"{
            "summary": "Found the main entry point",
            "artifacts": [{"path": "/src/main.rs", "relation": "read", "note": "entry point"}],
            "decisions": ["Use async runtime"],
            "nextSteps": ["Implement feature"],
            "details": null
        }"#;

        let result = parse_agent_output(json).expect("Should parse valid JSON");

        assert_eq!(result.summary, "Found the main entry point");
        assert_eq!(result.artifacts.len(), 1);
        assert_eq!(result.artifacts[0].path, "/src/main.rs");
        assert_eq!(result.artifacts[0].relation, "read");
        assert_eq!(result.decisions.len(), 1);
        assert_eq!(result.next_steps.len(), 1);
    }

    #[test]
    fn test_parse_agent_output_json_in_markdown() {
        let markdown = r#"Here's what I found:

```json
{
    "summary": "Analyzed the codebase",
    "artifacts": [],
    "decisions": ["Use Rust"],
    "nextSteps": [],
    "details": null
}
```

Hope this helps!"#;

        let result = parse_agent_output(markdown).expect("Should parse JSON from markdown");

        assert_eq!(result.summary, "Analyzed the codebase");
        assert_eq!(result.decisions.len(), 1);
        assert_eq!(result.decisions[0], "Use Rust");
    }

    #[test]
    fn test_parse_agent_output_fallback() {
        let plain_text = "I analyzed the code and found several issues.";

        let result = parse_agent_output(plain_text).expect("Should fallback gracefully");

        assert_eq!(result.summary, "Agent did not return structured output");
        assert!(result.artifacts.is_empty());
        assert!(result.decisions.is_empty());
        assert!(result.next_steps.is_empty());
        assert_eq!(result.details, Some(plain_text.to_string()));
    }

    #[test]
    fn test_extract_json_from_markdown_valid() {
        let markdown = r#"Some text
```json
{"key": "value"}
```
More text"#;

        let result = extract_json_from_markdown(markdown);

        assert_eq!(result, Some(r#"{"key": "value"}"#.to_string()));
    }

    #[test]
    fn test_extract_json_from_markdown_no_block() {
        let text = "Just plain text without code blocks";

        let result = extract_json_from_markdown(text);

        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_from_markdown_empty_block() {
        let markdown = r#"```json
```"#;

        let result = extract_json_from_markdown(markdown);

        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_from_markdown_unclosed() {
        let markdown = r#"```json
{"unclosed": true}
"#;

        let result = extract_json_from_markdown(markdown);

        assert!(result.is_none());
    }
}
