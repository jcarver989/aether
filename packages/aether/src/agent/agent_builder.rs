use crate::agent::Result;
use crate::agent::{Agent, AgentMessage, UserMessage};
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager, manager::McpServerConfig};
use crate::types::{ChatMessage, IsoString};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Handle for communicating with a running Agent
pub struct AgentHandle {
    handle: JoinHandle<()>,
    user_message_tx: mpsc::Sender<UserMessage>,
    agent_message_rx: mpsc::Receiver<AgentMessage>,
}

impl AgentHandle {
    /// Send a message to the agent
    pub async fn send(&mut self, message: UserMessage) -> Result<()> {
        self.user_message_tx
            .send(message)
            .await
            .map_err(|_| crate::agent::AgentError::Other("Agent channel closed".to_string()))
    }

    /// Receive a message from the agent
    pub async fn recv(&mut self) -> Option<AgentMessage> {
        self.agent_message_rx.recv().await
    }
}

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

    pub async fn spawn(self) -> Result<AgentHandle> {
        let mut messages = Vec::new();
        if let Some(system_prompt) = &self.system_prompt {
            messages.push(ChatMessage::System {
                content: system_prompt.clone(),
                timestamp: IsoString::now(),
            });
        }

        let (user_message_tx, user_message_rx) = mpsc::channel::<UserMessage>(100);
        let (agent_message_tx, agent_message_rx) = mpsc::channel::<AgentMessage>(100);
        let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(100);

        let mcp_manager = McpManager::new(elicitation_tx);
        mcp_manager.add_mcps(self.mcp_configs).await?;

        let agent = Agent::new(
            self.llm,
            mcp_manager,
            messages,
            user_message_rx,
            agent_message_tx,
        );
        let handle = tokio::spawn(agent.run());

        Ok(AgentHandle {
            handle,
            user_message_tx,
            agent_message_rx,
        })
    }
}
