use crate::mappers::map_mcp_prompt_to_available_command;
use aether::core::{AgentHandle, Prompt, agent};
use aether::events::{AgentMessage, UserMessage};
use aether::mcp::McpBuilder;
use aether::mcp::McpSpawnResult;
use aether::mcp::mcp;
use aether::mcp::run_mcp_task::McpCommand;
use llm::provider::StreamingModelProvider;
use mcp_utils::client::{ElicitationRequest, McpServerConfig, ServerInstructions};

use agent_client_protocol as acp;
use futures::FutureExt;
use mcp_servers::{
    CodingMcp, DefaultCodingTools, LspCodingTools, SkillsMcp, SubAgentsMcp, TasksMcp,
};
use mcp_utils::ServiceExt;
use std::path::{Path, PathBuf};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::debug;

/// Represents an active Aether agent session
pub struct Session {
    pub agent_tx: mpsc::Sender<UserMessage>,
    pub agent_rx: mpsc::Receiver<AgentMessage>,
    pub agent_handle: AgentHandle,
    pub _mcp_handle: JoinHandle<()>,
    pub mcp_tx: mpsc::Sender<McpCommand>,
    pub elicitation_rx: mpsc::Receiver<ElicitationRequest>,
}

impl Session {
    /// Creates a new session with the given LLM provider and configuration
    pub async fn new(
        llm: impl StreamingModelProvider + 'static,
        system_prompt: Option<String>,
        mcp_config_path: Option<PathBuf>,
        cwd: PathBuf,
        extra_mcp_servers: Vec<McpServerConfig>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        debug!("MCP config: {:?}", mcp_config_path);
        debug!("Using project root: {:?}", cwd);

        let roots_path = cwd.clone();
        let mut builder = build_mcp_servers(cwd, extra_mcp_servers, &roots_path);

        if let Some(ref config_path) = mcp_config_path {
            let config_str = config_path.to_str().ok_or("Invalid MCP config path")?;
            builder = builder.from_json_file(config_str).await?;
        }

        let McpSpawnResult {
            tool_definitions,
            instructions,
            command_tx: mcp_tx,
            elicitation_rx,
            handle: mcp_handle,
        } = builder.spawn().await?;

        let system_prompt =
            build_system_prompt(&roots_path, instructions, system_prompt.as_deref()).await?;

        let builder = agent(llm)
            .system_prompt(Prompt::text(&system_prompt))
            .tools(mcp_tx.clone(), tool_definitions);

        let (agent_tx, agent_rx, agent_handle) = builder.spawn().await?;

        Ok(Self {
            agent_tx,
            agent_rx,
            agent_handle,
            _mcp_handle: mcp_handle,
            mcp_tx,
            elicitation_rx,
        })
    }

    /// Lists available slash commands by querying MCP prompts
    pub async fn list_available_commands(
        &self,
    ) -> Result<Vec<acp::AvailableCommand>, Box<dyn std::error::Error>> {
        let (tx, rx) = oneshot::channel();

        self.mcp_tx
            .send(McpCommand::ListPrompts { tx })
            .await
            .map_err(|e| format!("Failed to send ListPrompts command: {e}"))?;

        let prompts = rx
            .await
            .map_err(|e| format!("Failed to receive prompts: {e}"))??;

        let commands = prompts
            .iter()
            .map(map_mcp_prompt_to_available_command)
            .collect();

        Ok(commands)
    }
}

fn build_mcp_servers(
    cwd: PathBuf,
    extra_mcp_servers: Vec<McpServerConfig>,
    roots_path: &Path,
) -> McpBuilder {
    let tasks_cwd = cwd.clone();
    mcp()
        .register_in_memory_server(
            "coding",
            Box::new(move |_args| {
                let project_path = cwd.clone();
                async move {
                    let lsp_tools =
                        LspCodingTools::new(DefaultCodingTools::new(), project_path.clone());
                    debug!("LspCodingTools created with lazy LSP spawning");
                    CodingMcp::with_tools(lsp_tools)
                        .with_root_dir(project_path)
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .register_in_memory_server(
            "skills",
            Box::new(|args| {
                async move {
                    SkillsMcp::from_args(args)
                        .expect("Failed to parse SkillsMcp args")
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .register_in_memory_server(
            "subagents",
            Box::new(|args| {
                async move {
                    SubAgentsMcp::from_args(args)
                        .expect("Failed to parse SubAgentsMcp args")
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .register_in_memory_server(
            "tasks",
            Box::new(move |args| {
                let project_path = tasks_cwd.clone();
                async move {
                    TasksMcp::from_args(args)
                        .unwrap_or_else(|e| {
                            tracing::warn!("Failed to parse TasksMcp args: {e}, using defaults");
                            TasksMcp::new(project_path)
                        })
                        .into_dyn()
                }
                .boxed()
            }),
        )
        .with_roots(vec![roots_path.to_path_buf()])
        .with_servers(extra_mcp_servers)
}

async fn build_system_prompt(
    roots_path: &Path,
    instructions: Vec<ServerInstructions>,
    custom_prompt: Option<&str>,
) -> Result<String, String> {
    let mut parts = vec![
        Prompt::agents_md().with_cwd(roots_path.to_path_buf()),
        Prompt::system_env().with_cwd(roots_path.to_path_buf()),
        Prompt::mcp_instructions(instructions),
    ];

    if let Some(custom) = custom_prompt {
        parts.push(Prompt::text(custom));
    }

    Prompt::build_all(&parts)
        .await
        .map_err(|e| format!("Failed to build system prompt: {e}"))
}
