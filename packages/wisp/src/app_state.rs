use std::error::Error;

use crate::cli::Cli;
use crate::cli::SystemPrompt;
use aether::mcp::McpServerConfig;
use aether::{
    agent::{AgentHandle, AgentMessage, UserMessage, agent},
    mcp::mcp,
};
use mcp_lexicon::CodingMcp;
use mcp_lexicon::ServiceExt;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

pub struct AppState {
    pub model_string: String,
    pub system_prompt: Option<SystemPrompt>,
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
        let system_prompt = cli.load_system_prompt();

        let mut agent_builder = agent(llm);
        if let Some(prompt) = &system_prompt {
            agent_builder = agent_builder.system(prompt.as_str());
        }

        let (tools, tx, mcp_handle) = mcp()
            .with_servers(vec![McpServerConfig::InMemory {
                name: "coding".to_string(),
                server: CodingMcp::new().into_dyn(),
            }])
            .spawn()
            .await?;

        let (agent_tx, agent_rx, agent_handle) = agent_builder.tools(tx, tools).spawn().await?;

        Ok(Self {
            model_string: cli.model.clone(),
            system_prompt,
            agent_tx,
            agent_rx,
            agent_handle,
            mcp_handle,
        })
    }
}
