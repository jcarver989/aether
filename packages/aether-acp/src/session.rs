use crate::mappers::map_mcp_prompt_to_available_command;
use aether::core::{AgentHandle, Prompt, agent};
use aether::events::{AgentMessage, UserMessage};
use aether::mcp::McpSpawnResult;
use aether::mcp::mcp;
use aether::mcp::run_mcp_task::McpCommand;
use llm::provider::StreamingModelProvider;
use mcp_utils::client::McpServerConfig;

use agent_client_protocol as acp;
use futures::FutureExt;
use mcp_coding::{CodingMcp, DefaultCodingTools, LspCodingTools};
use mcp_skills::SkillsMcp;
use mcp_subagents::SubAgentsMcp;
use mcp_tasks::TasksMcp;
use mcp_utils::ServiceExt;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::debug;

/// Represents an active Aether agent session
pub struct Session {
    pub id: String,
    pub agent_tx: mpsc::Sender<UserMessage>,
    pub agent_rx: mpsc::Receiver<AgentMessage>,
    #[allow(dead_code)]
    pub agent_handle: AgentHandle,
    pub _mcp_handle: JoinHandle<()>,
    pub mcp_tx: mpsc::Sender<McpCommand>,
}

impl Session {
    /// Creates a new session with the given LLM provider and configuration
    pub async fn new(
        id: String,
        llm: impl StreamingModelProvider + 'static,
        system_prompt: Option<String>,
        mcp_config_path: Option<PathBuf>,
        cwd: PathBuf,
        extra_mcp_servers: Vec<McpServerConfig>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        debug!("Creating new session: {}", id);
        debug!("MCP config: {:?}", mcp_config_path);
        debug!("Using project root: {:?}", cwd);

        let tasks_cwd = cwd.clone();
        let roots_path = cwd.clone();
        let mut builder = mcp()
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
                                tracing::warn!(
                                    "Failed to parse TasksMcp args: {e}, using defaults"
                                );
                                TasksMcp::new(project_path)
                            })
                            .into_dyn()
                    }
                    .boxed()
                }),
            )
            .with_roots(vec![roots_path.clone()])
            .with_servers(extra_mcp_servers);

        if let Some(ref config_path) = mcp_config_path {
            let config_str = config_path.to_str().ok_or("Invalid MCP config path")?;
            builder = builder.from_json_file(config_str).await?;
        }

        let McpSpawnResult {
            tool_definitions,
            instructions,
            command_tx: mcp_tx,
            handle: mcp_handle,
        } = builder.spawn().await?;

        let system_prompt = {
            let mut parts = vec![
                Prompt::agents_md().with_cwd(roots_path.clone()),
                Prompt::system_env().with_cwd(roots_path),
                Prompt::mcp_instructions(instructions),
            ];

            if let Some(ref custom_prompt) = system_prompt {
                parts.push(Prompt::text(custom_prompt));
            }

            Prompt::build_all(&parts)
                .await
                .map_err(|e| format!("Failed to build system prompt: {e}"))?
        };

        let builder = agent(llm)
            .system_prompt(Prompt::text(&system_prompt))
            .tools(mcp_tx.clone(), tool_definitions);

        let (agent_tx, agent_rx, agent_handle) = builder.spawn().await?;

        debug!("Session {} created successfully", id);

        Ok(Self {
            id,
            agent_tx,
            agent_rx,
            agent_handle,
            _mcp_handle: mcp_handle,
            mcp_tx,
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
