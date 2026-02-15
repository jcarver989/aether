use crate::cli::Cli;
use aether::mcp::McpServerConfig;
use aether::mcp::oauth::BrowserOAuthHandler;
use aether::{
    agent::{AgentHandle, Prompt, agent},
    mcp::{McpSpawnResult, mcp},
};
use agent_events::{AgentMessage, UserMessage};
use mcp_lexicon::coding::{DefaultCodingTools, LspCodingTools};
use mcp_lexicon::{CodingMcp, PluginsMcp, ServiceExt, TasksMcp};
use std::env::current_dir;
use std::error::Error;
use std::path::PathBuf;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

pub struct AppState {
    #[allow(dead_code)]
    pub model_string: String,
    pub agent_tx: Sender<UserMessage>,
    pub agent_rx: Receiver<AgentMessage>,
    #[allow(dead_code)]
    agent_handle: AgentHandle,

    #[allow(dead_code)]
    mcp_handle: JoinHandle<()>,
}

impl AppState {
    pub async fn from_cli(cli: &Cli) -> Result<Self, Box<dyn Error>> {
        let llm = cli.load_model_provider()?;

        let system_prompt = {
            let mut parts = Vec::new();
            if let Some(prompt) = &cli.load_system_prompt() {
                parts.push(Prompt::text(prompt.as_str()));
            }
            parts.push(Prompt::system_env());
            Prompt::build_all(&parts)?
        };

        let agent_builder = agent(llm).system(&system_prompt);
        let root_path = current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let lsp_tools = LspCodingTools::new(DefaultCodingTools::new(), root_path.clone());

        // Get ~/.aether path for plugins
        let aether_dir = dirs::home_dir()
            .map(|h| h.join(".aether"))
            .unwrap_or_else(|| PathBuf::from(".aether"));

        let McpSpawnResult {
            tool_definitions,
            instructions,
            command_tx,
            handle: mcp_handle,
        } = mcp()
            .with_oauth_handler(BrowserOAuthHandler::new()?)
            .with_servers(vec![
                McpServerConfig::InMemory {
                    name: "coding".to_string(),
                    server: CodingMcp::with_tools(lsp_tools).into_dyn(),
                },
                McpServerConfig::InMemory {
                    name: "plugins".to_string(),
                    server: PluginsMcp::new(aether_dir).into_dyn(),
                },
                McpServerConfig::InMemory {
                    name: "tasks".to_string(),
                    server: TasksMcp::new(root_path.clone()).into_dyn(),
                },
            ])
            .spawn()
            .await?;

        let (agent_tx, agent_rx, agent_handle) = agent_builder
            .mcp_instructions(instructions)
            .tools(command_tx, tool_definitions)
            .spawn()
            .await?;

        Ok(Self {
            model_string: cli.model.clone(),
            agent_tx,
            agent_rx,
            agent_handle,
            mcp_handle,
        })
    }
}
