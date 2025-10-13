use crate::agent::Result;
use crate::agent::middleware::{AgentEvent, Middleware, MiddlewareAction};
use crate::agent::{Agent, AgentMessage, UserMessage};
use crate::llm::{Context, StreamingModelProvider};
use crate::mcp::run_mcp_task::McpCommand;
use crate::types::{ChatMessage, IsoString, ToolDefinition};
use std::future::Future;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::JoinHandle;

/// Handle for communicating with a running Agent
pub struct AgentHandle {
    _agent_handle: JoinHandle<()>,
}

pub struct AgentBuilder<T: StreamingModelProvider> {
    llm: T,
    system_prompt: Option<String>,
    middleware: Middleware,
    tool_definitions: Vec<ToolDefinition>,
    mcp_tx: Option<Sender<McpCommand>>,
}

impl<T: StreamingModelProvider + 'static> AgentBuilder<T> {
    pub fn new(llm: T) -> Self {
        Self {
            llm,
            system_prompt: None,
            middleware: Middleware::new(),
            tool_definitions: Vec::new(),
            mcp_tx: None,
        }
    }

    pub fn system(mut self, prompt: &str) -> Self {
        self.system_prompt = Some(prompt.to_string());
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

    pub fn mcp_tools(mut self, tx: Sender<McpCommand>, tools: Vec<ToolDefinition>) -> Self {
        self.tool_definitions = tools;
        self.mcp_tx = Some(tx);
        self
    }

    pub async fn spawn(self) -> Result<(Sender<UserMessage>, Receiver<AgentMessage>, AgentHandle)> {
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
        let context = Context::new(messages, self.tool_definitions);

        let agent = Agent::new(
            self.llm,
            context,
            self.mcp_tx,
            user_message_rx,
            agent_message_tx,
            self.middleware,
        );

        let agent_handle = tokio::spawn(agent.run());

        Ok((
            user_message_tx,
            agent_message_rx,
            AgentHandle {
                _agent_handle: agent_handle,
            },
        ))
    }
}
