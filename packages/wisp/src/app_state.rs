use std::error::Error;
use std::sync::Arc;

use crate::cli::SystemPrompt;
use crate::cli::{Cli, ModelSpec};
use aether::agent::{AgentMessage, SpawnedAgent, UserMessage, agent};
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

pub struct AppState {
    pub model_specs: Vec<ModelSpec>,
    pub system_prompt: Option<SystemPrompt>,
    pub agent_handle: JoinHandle<()>,
    pub agent_tx: Sender<UserMessage>,
    pub agent_rx: Arc<Mutex<Receiver<AgentMessage>>>,
}

impl AppState {
    pub async fn from_cli(cli: &Cli) -> Result<Self, Box<dyn Error>> {
        let (llm, model_specs) = cli.load_model_provider()?;
        let system_prompt = cli.load_system_prompt();

        let mut agent_builder = agent(llm);
        if let Some(prompt) = &system_prompt {
            agent_builder = agent_builder.system_prompt(prompt.as_str());
        }

        let agent = agent_builder.spawn().await?;

        Ok(Self {
            model_specs,
            system_prompt,
            agent_handle: agent.task_handle,
            agent_tx: agent.tx,
            agent_rx: Arc::new(Mutex::new(agent.rx)),
        })
    }
}

#[derive(Debug, Clone)]
pub enum ChatMessage {
    Assistant { message_id: String, text: String },
    User { text: String },
}
