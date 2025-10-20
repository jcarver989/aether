use aether::agent::{AgentHandle, AgentMessage, Prompt, UserMessage, agent};
use aether::llm::provider::StreamingModelProvider;
use aether::mcp::mcp;
use mcp_lexicon::{CodingMcp, ServiceExt};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::debug;

/// Represents an active Aether agent session
pub struct Session {
    pub id: String,
    pub agent_tx: mpsc::Sender<UserMessage>,
    pub agent_rx: mpsc::Receiver<AgentMessage>,
    pub agent_handle: AgentHandle,
    pub mcp_handle: JoinHandle<()>,
    pub cancel_flag: Arc<AtomicBool>,
}

impl Session {
    /// Creates a new session with the given LLM provider and configuration
    pub async fn new<T: StreamingModelProvider + 'static>(
        id: String,
        llm: T,
        system_prompt: Option<String>,
        mcp_config_path: std::path::PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        debug!("Creating new session: {}", id);
        debug!("Loading MCP configuration from: {:?}", mcp_config_path);

        // Register the coding server factory
        let config_str = mcp_config_path.to_str().ok_or("Invalid MCP config path")?;
        let (tools, mcp_tx, mcp_handle) = mcp()
            .register_in_memory_server("coding", Box::new(|| CodingMcp::new().into_dyn()))
            .from_json_file(config_str)?
            .spawn()
            .await?;

        // Build system prompt from AGENTS.md and optional custom prompt
        let mut prompts = vec![Prompt::agents_md()];
        if let Some(ref custom_prompt) = system_prompt {
            prompts.push(Prompt::text(custom_prompt));
        }

        let system_prompt_text = Prompt::build_all(&prompts)
            .map_err(|e| format!("Failed to build system prompt: {}", e))?;

        let builder = agent(llm)
            .system(&system_prompt_text)
            .tools(mcp_tx, tools);

        let (agent_tx, agent_rx, agent_handle) = builder.spawn().await?;

        debug!("Session {} created successfully", id);

        Ok(Self {
            id,
            agent_tx,
            agent_rx,
            agent_handle,
            mcp_handle,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Sends a text prompt to the agent
    pub async fn send_prompt(&self, text: String) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Sending prompt to session {}", self.id);
        self.agent_tx
            .send(UserMessage::text(&text))
            .await
            .map_err(|e| format!("Failed to send prompt: {}", e))?;
        Ok(())
    }

    /// Cancels any ongoing prompt processing
    pub async fn cancel(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Cancelling session {}", self.id);
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.agent_tx
            .send(UserMessage::Cancel)
            .await
            .map_err(|e| format!("Failed to send cancel: {}", e))?;
        Ok(())
    }

    /// Receives the next agent message
    pub async fn recv(&mut self) -> Option<AgentMessage> {
        self.agent_rx.recv().await
    }

    /// Checks if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        debug!("Dropping session {}", self.id);
        // The agent_handle and channels will be dropped, which will clean up the agent task
    }
}
