use aether::events::AgentMessage;
use aether::events::SubAgentProgressPayload;
use clap::Parser;
use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ProgressNotificationParam, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::tools::{
    AgentExecutor, ListAgentsOutput, SpawnSubAgentsInput, SpawnSubAgentsOutput, SubAgentListItem,
};
use crate::subagent_file::{AgentFile, SubAgentInfo, load_agent_metadata};

/// Callback type for reporting agent progress during subagent execution.
type ProgressCallback = Box<dyn Fn(&str, &str, &AgentMessage) + Send + Sync>;

/// CLI arguments for `SubAgentsMcp` server
#[derive(Debug, Clone, Parser)]
pub struct SubAgentsMcpArgs {
    /// Base directory for sub-agents (contains 'sub-agents' subdirectory)
    #[arg(long = "dir")]
    pub base_dir: Option<PathBuf>,
}

impl SubAgentsMcpArgs {
    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let mut full_args = vec!["subagents-mcp".to_string()];
        full_args.extend(args);

        Self::try_parse_from(full_args)
            .map_err(|e| format!("Failed to parse SubAgentsMcp arguments: {e}"))
    }
}

/// MCP server for sub-agent management and execution
#[derive(Clone)]
pub struct SubAgentsMcp {
    agents_dir: PathBuf,
    agents_info: Vec<SubAgentInfo>,
    tool_router: ToolRouter<Self>,
    roots: Arc<RwLock<Vec<PathBuf>>>,
}

impl SubAgentsMcp {
    pub fn new(base_dir: PathBuf) -> Self {
        let agents_dir = base_dir.join("sub-agents");
        let agents_info = load_agent_metadata(&agents_dir);

        Self {
            agents_dir,
            agents_info,
            tool_router: Self::tool_router(),
            roots: Arc::new(RwLock::new(vec![base_dir])),
        }
    }

    pub fn from_args(args: Vec<String>) -> Result<Self, String> {
        let parsed_args = SubAgentsMcpArgs::from_args(args)?;
        let base_dir = parsed_args.base_dir.unwrap_or_else(|| PathBuf::from("."));
        Ok(Self::new(base_dir))
    }

    pub fn with_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.roots = Arc::new(RwLock::new(roots));
        self
    }

    fn build_instructions(&self) -> String {
        let mut instructions = include_str!("./instructions.md").to_string();

        if !self.agents_info.is_empty() {
            instructions.push_str("\n\n## Available Sub-Agents\n");
            instructions.push_str("The following sub-agents are available:\n\n");

            for agent in &self.agents_info {
                use std::fmt::Write as _;
                let _ = writeln!(instructions, "- **{}**: {}", agent.name, agent.description);
            }
        }

        instructions
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SubAgentsMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "subagents-mcp".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(self.build_instructions()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool_router]
impl SubAgentsMcp {
    #[doc = include_str!("tools/list_subagents/description.md")]
    #[tool]
    pub async fn list_subagents(&self) -> Result<Json<ListAgentsOutput>, String> {
        let agents_with_dirs =
            match AgentFile::from_nested_dirs(&self.agents_dir, "AGENTS.md").await {
                Ok(agents) => agents,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
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

    #[doc = include_str!("tools/spawn_subagent/description.md")]
    #[tool]
    pub async fn spawn_subagent(
        &self,
        request: Parameters<SpawnSubAgentsInput>,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<SpawnSubAgentsOutput>, String> {
        let Parameters(args) = request;

        // Set up MCP progress notifications
        let progress_token = context.meta.get_progress_token();
        let peer = Arc::new(context.peer.clone());
        let message_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));

        let progress_callback: ProgressCallback = {
            let progress_token = progress_token.clone();
            let peer = Arc::clone(&peer);
            let message_counter = Arc::clone(&message_counter);

            Box::new(
                move |task_id: &str, agent_name: &str, message: &AgentMessage| {
                    if let Some(ref token) = progress_token {
                        let counter =
                            message_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let progress_payload = SubAgentProgressPayload {
                            task_id: task_id.to_string(),
                            agent_name: agent_name.to_string(),
                            event: message.clone(),
                        };

                        let peer = Arc::clone(&peer);
                        let token = token.clone();
                        let progress_data_str =
                            serde_json::to_string(&progress_payload).unwrap_or_default();

                        tokio::spawn(async move {
                            let _ = peer
                                .notify_progress(ProgressNotificationParam {
                                    progress_token: token,
                                    #[allow(clippy::cast_precision_loss)]
                                    progress: counter as f64,
                                    total: None,
                                    message: Some(progress_data_str),
                                })
                                .await;
                        });
                    }
                },
            )
        };

        // Pass inherited roots to sub-agents
        let roots = self.roots.read().await.clone();
        let executor = AgentExecutor::new(self.agents_dir.clone(), roots)
            .with_progress_callback(progress_callback);

        let output = executor.execute_tasks(args.tasks).await;
        Ok(Json(output))
    }
}
