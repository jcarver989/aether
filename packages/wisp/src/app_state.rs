use std::error::Error;

use crate::cli::SystemPrompt;
use crate::cli::{Cli, ModelSpec};
use aether::agent::{SpawnedAgent, agent};
use mcp_lexicon::AgentBuilderExt;

pub struct AppState {
    pub model_specs: Vec<ModelSpec>,
    pub agent: SpawnedAgent,
    pub system_prompt: Option<SystemPrompt>,
}

impl AppState {
    pub async fn from_cli(cli: &Cli) -> Result<Self, Box<dyn Error>> {
        let (llm, model_specs) = cli.load_model_provider()?;
        let mut builder = agent(llm);
        let system_prompt = cli.load_system_prompt();

        if let Some(prompt) = &system_prompt {
            builder = builder.system_prompt(prompt.as_str());
        }

        Ok(Self {
            agent: builder.coding_tools().spawn().await?,
            model_specs,
            system_prompt,
        })
    }
}
