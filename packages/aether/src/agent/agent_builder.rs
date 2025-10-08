use crate::agent::Result;
use crate::agent::middleware::{AgentEvent, Middleware, MiddlewareAction};
use crate::agent::{Agent, AgentMessage, UserMessage};
use crate::llm::{Context, StreamingModelProvider};
use crate::mcp::run_mcp_task::{McpCommand, McpEvent, run_mcp_task};
use crate::mcp::{ElicitationRequest, McpManager, manager::McpServerConfig};
use crate::types::{ChatMessage, IsoString};
use std::future::Future;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Handle for communicating with a running Agent
pub struct AgentHandle {
    _agent_handle: JoinHandle<()>,
    _mcp_handle: JoinHandle<()>,
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

pub struct AgentBuilder<T: StreamingModelProvider> {
    llm: T,
    system_prompt: Option<String>,
    mcp_configs: Vec<McpServerConfig>,
    middleware: Middleware,
}

impl<T: StreamingModelProvider + 'static> AgentBuilder<T> {
    pub fn new(llm: T) -> Self {
        Self {
            llm,
            system_prompt: None,
            mcp_configs: Vec::new(),
            middleware: Middleware::new(),
        }
    }

    pub fn system(mut self, prompt: &str) -> Self {
        self.system_prompt = Some(prompt.to_string());
        self
    }

    pub fn mcp(mut self, config: McpServerConfig) -> Self {
        self.mcp_configs.push(config);
        self
    }

    /// Add an event handler for agent events
    ///
    /// Handlers receive all agent events and can pattern match to handle specific ones.
    /// Multiple handlers can be registered and will execute in parallel.
    /// If any handler returns Block, the action will be blocked.
    ///
    /// # Example
    /// ```ignore
    /// agent(llm)
    ///     .on_event(|event| async move {
    ///         match event {
    ///             AgentEvent::ToolCall { name, .. } if name == "dangerous_tool" => {
    ///                 MiddlewareAction::Block
    ///             }
    ///             AgentEvent::AgentDone => {
    ///                 println!("Done!");
    ///                 MiddlewareAction::Allow
    ///             }
    ///             _ => MiddlewareAction::Allow
    ///         }
    ///     })
    /// ```
    pub fn on_event<U, V>(mut self, handler: U) -> Self
    where
        U: Fn(AgentEvent) -> V + Send + Sync + 'static,
        V: Future<Output = MiddlewareAction> + Send + 'static,
    {
        self.middleware.add_handler(handler);
        self
    }

    pub async fn spawn(self) -> Result<AgentHandle> {
        let mut messages = Vec::new();
        if let Some(content) = self.system_prompt {
            messages.push(ChatMessage::System {
                content,
                timestamp: IsoString::now(),
            });
        }

        let queue_size = 100;
        let (user_message_tx, user_message_rx) = mpsc::channel::<UserMessage>(queue_size);
        let (agent_message_tx, agent_message_rx) = mpsc::channel::<AgentMessage>(queue_size);
        let (mcp_command_tx, mcp_command_rx) = mpsc::channel::<McpCommand>(queue_size);
        let (mcp_event_tx, mcp_event_rx) = mpsc::channel::<McpEvent>(queue_size);
        let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(queue_size);

        let mut mcp_manager = McpManager::new(elicitation_tx);
        mcp_manager.add_mcps(self.mcp_configs).await?;

        let tool_definitions = mcp_manager.tool_definitions();
        let context = Context::new(messages, tool_definitions);

        let agent = Agent::new(
            self.llm,
            context,
            mcp_command_tx,
            mcp_event_rx,
            user_message_rx,
            agent_message_tx,
            self.middleware,
        );

        let mcp_handle = tokio::spawn(run_mcp_task(mcp_manager, mcp_command_rx, mcp_event_tx));
        let agent_handle = tokio::spawn(agent.run());

        Ok(AgentHandle {
            _agent_handle: agent_handle,
            _mcp_handle: mcp_handle,
            user_message_tx,
            agent_message_rx,
        })
    }
}
