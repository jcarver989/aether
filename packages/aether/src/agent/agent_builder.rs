use crate::agent::Result;
use crate::agent::{Agent, AgentMessage, UserMessage};
use crate::llm::{Context, StreamingModelProvider};
use crate::mcp::run_mcp_task::{McpCommand, McpEvent, run_mcp_task};
use crate::mcp::{ElicitationRequest, McpManager, manager::McpServerConfig};
use crate::types::{ChatMessage, IsoString};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::AgentError;

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
    system_prompts: Vec<SystemPrompt>,
    mcp_configs: Vec<McpServerConfig>,
}

impl<T: StreamingModelProvider + 'static> AgentBuilder<T> {
    pub fn new(llm: T) -> Self {
        Self {
            llm,
            system_prompts: Vec::new(),
            mcp_configs: Vec::new(),
        }
    }

    pub fn system(mut self, prompts: &[SystemPrompt]) -> Self {
        self.system_prompts = prompts.to_vec();
        self
    }

    pub fn mcp(mut self, config: McpServerConfig) -> Self {
        self.mcp_configs.push(config);
        self
    }

    pub async fn spawn(self) -> Result<AgentHandle> {
        let mut messages = Vec::new();
        if !self.system_prompts.is_empty() {
            let content: Result<Vec<_>> = self.system_prompts.iter().map(|p| p.resolve()).collect();
            messages.push(ChatMessage::System {
                content: content?.join("\n\n"),
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

#[derive(Debug, Clone)]
pub enum SystemPrompt {
    Text(String),
    File { path: String, ancestors: bool },
}

impl SystemPrompt {
    pub fn text(str: &str) -> Self {
        Self::Text(str.to_string())
    }

    pub fn file(path: &str, ancestors: bool) -> Self {
        Self::File {
            path: path.to_string(),
            ancestors,
        }
    }

    fn resolve(&self) -> Result<String> {
        match self {
            SystemPrompt::Text(text) => Ok(text.clone()),
            SystemPrompt::File { path, ancestors } => {
                if *ancestors {
                    Self::resolve_file_with_ancestors(path)
                } else {
                    Self::resolve_file(&PathBuf::from(path))
                }
            }
        }
    }

    fn resolve_file(path: &Path) -> Result<String> {
        fs::read_to_string(path).map_err(|e| {
            AgentError::IoError(format!("Failed to read file '{}': {}", path.display(), e))
        })
    }

    fn resolve_file_with_ancestors(filename: &str) -> Result<String> {
        let mut prompt = Vec::new();
        let mut current_dir = env::current_dir()
            .map_err(|e| AgentError::IoError(format!("Failed to get current directory: {}", e)))?;

        loop {
            let file_path = current_dir.join(filename);
            if file_path.exists() && file_path.is_file() {
                let content = Self::resolve_file(&file_path)?;
                prompt.push(content);
            }

            match current_dir.parent() {
                Some(parent) => {
                    // Stop before root (/)
                    if parent.parent().is_none() {
                        break;
                    }
                    current_dir = parent.to_path_buf();
                }
                None => break,
            }
        }

        if prompt.is_empty() {
            return Err(AgentError::IoError(format!(
                "No '{}' files found in directory tree",
                filename
            )));
        }

        // Want root -> CWD (i.e. general --> specific prompt)
        prompt.reverse();
        Ok(prompt.join("\n\n"))
    }
}
