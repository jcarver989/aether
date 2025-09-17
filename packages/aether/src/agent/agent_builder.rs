use crate::agent::Agent;
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager, manager::McpServerConfig};
use crate::types::{ChatMessage, IsoString};
use color_eyre::Result;
use tokio::sync::mpsc;

pub struct AgentBuilder<T: ModelProvider> {
    llm: T,
    system_prompt: Option<String>,
    mcp_configs: Vec<McpServerConfig>,
}

impl<T: ModelProvider + 'static> AgentBuilder<T> {
    pub fn new(llm: T) -> Self {
        Self {
            llm,
            system_prompt: None,
            mcp_configs: Vec::new(),
        }
    }

    pub fn system_prompt(mut self, prompt: &str) -> Self {
        if prompt.is_empty() {
            return self;
        }

        self.system_prompt = Some(prompt.to_string());
        self
    }

    pub fn mcp(mut self, config: McpServerConfig) -> Self {
        self.mcp_configs.push(config);
        self
    }

    pub async fn build(self) -> Result<Agent<T>> {
        let mut messages = Vec::new();

        if let Some(system_prompt) = &self.system_prompt {
            messages.push(ChatMessage::System {
                content: system_prompt.clone(),
                timestamp: IsoString::now(),
            });
        }

        let (elicitation_tx, elicitation_rx) = mpsc::unbounded_channel::<ElicitationRequest>();
        let mut mcp_manager = McpManager::new(elicitation_tx);

        for config in self.mcp_configs {
            mcp_manager.add_mcp(config).await?;
        }

        Ok(Agent::new(self.llm, mcp_manager, messages, elicitation_rx))
    }

}
