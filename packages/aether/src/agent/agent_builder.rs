use crate::agent::Agent;
use crate::agent::{AgentMessage, UserMessage};
use crate::llm::ModelProvider;
use crate::mcp::{McpManager, manager::McpServerConfig};
use crate::types::{ChatMessage, IsoString};
use color_eyre::Result;
use futures::StreamExt;
use futures::pin_mut;
use tokio::sync::mpsc;

pub struct AgentBuilder<T: ModelProvider> {
    llm: T,
    system_prompt: Option<String>,
    mcp_manager: McpManager,
    mcp_configs: Vec<McpServerConfig>,
}

impl<T: ModelProvider + 'static> AgentBuilder<T> {
    pub fn new(llm: T) -> Self {
        Self {
            llm,
            system_prompt: None,
            mcp_manager: McpManager::new(),
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

        let mut mcp_manager = self.mcp_manager;

        for config in self.mcp_configs {
            mcp_manager.add_mcp(config).await?;
        }

        Ok(Agent::new(self.llm, mcp_manager, messages))
    }

    pub async fn spawn(self) -> Result<(mpsc::Sender<UserMessage>, mpsc::Receiver<AgentMessage>)> {
        let (user_tx, mut user_rx) = mpsc::channel::<UserMessage>(100);
        let (agent_tx, agent_rx) = mpsc::channel::<AgentMessage>(100);

        let mut agent = self.build().await?;

        tokio::spawn(async move {
            while let Some(message) = user_rx.recv().await {
                let (result_stream, _cancel_token) = agent.send(message).await;
                pin_mut!(result_stream);

                while let Some(event) = result_stream.next().await {
                    if agent_tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
        });

        Ok((user_tx, agent_rx))
    }
}
