use crate::agent::Result;
use crate::agent::{Agent, AgentMessage, UserMessage};
use crate::llm::ModelProvider;
use crate::mcp::mcp_task::McpManagerTask;
use crate::mcp::{ElicitationRequest, McpManager, manager::McpServerConfig};
use crate::types::{ChatMessage, IsoString};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Handle for communicating with a running Agent
pub struct AgentHandle {
    agent_handle: JoinHandle<()>,
    mcp_task_handle: JoinHandle<()>,
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

    /// Wait for both agent and MCP tasks to complete
    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        let (agent_result, mcp_result) = tokio::join!(self.agent_handle, self.mcp_task_handle);
        agent_result?;
        mcp_result?;
        Ok(())
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

        let mut mcp_manager = McpManager::new(elicitation_tx);
        mcp_manager.add_mcps(self.mcp_configs).await?;
        let initial_tools = mcp_manager.tool_definitions();

        let context = crate::llm::Context::new(messages, initial_tools);

        let (mcp_task, mcp_event_stream) = McpManagerTask::new(mcp_manager);
        let mcp_command_tx = mcp_task.command_tx.clone();
        let mcp_task_handle = mcp_task.handle;

        let agent = Agent::new(
            self.llm,
            context,
            mcp_command_tx,
            mcp_event_stream,
            user_message_rx,
            agent_message_tx,
        );
        let agent_handle = tokio::spawn(agent.run());

        Ok(AgentHandle {
            agent_handle,
            mcp_task_handle,
            user_message_tx,
            agent_message_rx,
        })
    }
}
