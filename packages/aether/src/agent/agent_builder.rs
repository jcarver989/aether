use crate::agent::{Agent, AgentMessage, UserMessage};
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager, manager::McpServerConfig};
use crate::types::{ChatMessage, IsoString};
use crate::agent::Result;
use futures::{StreamExt, pin_mut};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

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

        let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(50);
        let mut mcp_manager = McpManager::new(elicitation_tx);

        for config in self.mcp_configs {
            mcp_manager.add_mcp(config).await?;
        }

        Ok(Agent::new(self.llm, mcp_manager, messages))
    }

    pub async fn spawn(self) -> Result<SpawnedAgent> {
        let (user_message_tx, mut user_message_rx) = mpsc::channel::<UserMessage>(100);
        let (agent_message_tx, agent_message_rx) = mpsc::channel::<AgentMessage>(100);
        let mut agent = self.build().await?;

        let task_handle = tokio::spawn(async move {
            while let Some(user_message) = user_message_rx.recv().await {
                let response_stream = agent.send(user_message).await;
                pin_mut!(response_stream);

                while let Some(agent_message) = response_stream.next().await {
                    if agent_message_tx.send(agent_message).await.is_err() {
                        tracing::debug!("Agent message receiver dropped, terminating agent task");
                        return;
                    }
                }
            }

            tracing::debug!("User message sender dropped, terminating agent task");
        });

        Ok(SpawnedAgent {
            task_handle,
            tx: user_message_tx,
            rx: agent_message_rx,
        })
    }
}

pub struct SpawnedAgent {
    pub task_handle: JoinHandle<()>,
    pub tx: mpsc::Sender<UserMessage>,
    pub rx: mpsc::Receiver<AgentMessage>,
}
