use std::error::Error;
use std::sync::Arc;

use crate::cli::SystemPrompt;
use crate::cli::{Cli, ModelSpec};
use aether::agent::{AgentHandle, agent};
use mcp_lexicon::AgentBuilderExt;
use tokio::sync::Mutex;

pub struct AppState {
    pub model_specs: Vec<ModelSpec>,
    pub system_prompt: Option<SystemPrompt>,
    pub agent: Arc<Mutex<AgentHandle>>,
}

impl AppState {
    pub async fn from_cli(cli: &Cli) -> Result<Self, Box<dyn Error>> {
        let (llm, model_specs) = cli.load_model_provider()?;
        let system_prompt = cli.load_system_prompt();

        let mut agent_builder = agent(llm);
        if let Some(prompt) = &system_prompt {
            agent_builder = agent_builder.system_prompt(prompt.as_str());
        }

        let agent = agent_builder.coding_tools().spawn().await?;

        Ok(Self {
            model_specs,
            system_prompt,
            agent: Arc::new(Mutex::new(agent)),
        })
    }
}

#[derive(Debug, Clone)]
pub enum ChatMessage {
    Assistant {
        message_id: String,
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: Option<String>,
        result: Option<String>,
        model_name: String,
        is_complete: bool,
    },
    User {
        text: String,
    },
}
