use aether::agent::{AgentHandle, AgentMessage, Prompt, UserMessage, agent};
use aether::llm::provider::StreamingModelProvider;
use aether::mcp::mcp;
use aether::mcp::run_mcp_task::McpCommand;
use agent_client_protocol as acp;
use mcp_lexicon::{CodingMcp, PluginsMcp, ServiceExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::debug;

use crate::acp_actor::AcpActorHandle;
use crate::acp_coding_tools::AcpCodingTools;
use crate::mappers::map_mcp_prompt_to_available_command;

/// Represents an active Aether agent session
pub struct Session {
    pub id: String,
    pub agent_tx: mpsc::Sender<UserMessage>,
    pub agent_rx: mpsc::Receiver<AgentMessage>,
    pub _agent_handle: AgentHandle,
    pub _mcp_handle: JoinHandle<()>,
    pub cancel_flag: Arc<AtomicBool>,
    pub mcp_tx: mpsc::Sender<McpCommand>,
}

impl Session {
    /// Creates a new session with the given LLM provider and configuration
    pub async fn new<T: StreamingModelProvider + 'static>(
        id: String,
        llm: T,
        system_prompt: Option<String>,
        mcp_config_path: std::path::PathBuf,
        acp_info: Option<(AcpActorHandle, acp::SessionId)>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        debug!("Creating new session: {}", id);
        debug!("Loading MCP configuration from: {:?}", mcp_config_path);

        // Register the coding and slash-commands server factories
        let config_str = mcp_config_path.to_str().ok_or("Invalid MCP config path")?;

        // Determine prompts directory - use ~/.aether/prompts or a default location
        let prompts_dir = dirs::home_dir()
            .map(|home| home.join(".aether").join("prompts"))
            .unwrap_or_else(|| PathBuf::from("prompts"));

        let (tools, mcp_tx, mcp_handle) = if let Some((actor_handle, session_id)) = acp_info {
            // Use ACP-enabled CodingMcp
            debug!("Creating ACP-enabled CodingMcp and SlashCommandMcp");

            mcp()
                .register_in_memory_server(
                    "coding",
                    Box::new(move || {
                        let tools = AcpCodingTools::new(actor_handle.clone(), session_id.clone());
                        CodingMcp::with_tools(tools).into_dyn()
                    }),
                )
                .register_in_memory_server(
                    "plugins",
                    Box::new(move || PluginsMcp::new(prompts_dir.clone()).into_dyn()),
                )
                .from_json_file(config_str)?
                .spawn()
                .await?
        } else {
            // Use default (local filesystem) CodingMcp
            debug!("Creating default CodingMcp and SlashCommandMcp");
            mcp()
                .register_in_memory_server("coding", Box::new(|| CodingMcp::new().into_dyn()))
                .register_in_memory_server(
                    "slash-commands",
                    Box::new(move || PluginsMcp::new(prompts_dir.clone()).into_dyn()),
                )
                .from_json_file(config_str)?
                .spawn()
                .await?
        };

        // Build system prompt from AGENTS.md and optional custom prompt
        let mut prompts = vec![Prompt::agents_md()];
        if let Some(ref custom_prompt) = system_prompt {
            prompts.push(Prompt::text(custom_prompt));
        }

        let system_prompt_text = Prompt::build_all(&prompts)
            .map_err(|e| format!("Failed to build system prompt: {}", e))?;

        let builder = agent(llm)
            .system(&system_prompt_text)
            .tools(mcp_tx.clone(), tools);

        let (agent_tx, agent_rx, agent_handle) = builder.spawn().await?;

        debug!("Session {} created successfully", id);

        Ok(Self {
            id,
            agent_tx,
            agent_rx,
            _agent_handle: agent_handle,
            _mcp_handle: mcp_handle,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            mcp_tx,
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

    /// Lists available slash commands by querying MCP prompts
    pub async fn list_available_commands(
        &self,
    ) -> Result<Vec<acp::AvailableCommand>, Box<dyn std::error::Error>> {
        let (tx, rx) = oneshot::channel();

        self.mcp_tx
            .send(McpCommand::ListPrompts { tx })
            .await
            .map_err(|e| format!("Failed to send ListPrompts command: {}", e))?;

        let prompts = rx
            .await
            .map_err(|e| format!("Failed to receive prompts: {}", e))??;

        let commands = prompts
            .iter()
            .map(map_mcp_prompt_to_available_command)
            .collect();

        Ok(commands)
    }

    /// Expands a slash command by fetching the corresponding MCP prompt
    pub async fn expand_slash_command(
        &self,
        command_name: &str,
        args_text: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Parse arguments as positional parameters
        let arguments = if args_text.is_empty() {
            None
        } else {
            let mut arg_map = serde_json::Map::new();
            for (i, arg) in args_text.split_whitespace().enumerate() {
                arg_map.insert(i.to_string(), serde_json::Value::String(arg.to_string()));
            }
            Some(arg_map)
        };

        // Try to find the prompt by querying all available prompts
        // We need to map the command name back to its namespaced form
        let (tx_list, rx_list) = oneshot::channel();
        self.mcp_tx
            .send(McpCommand::ListPrompts { tx: tx_list })
            .await
            .map_err(|e| format!("Failed to send ListPrompts command: {}", e))?;

        let prompts = rx_list
            .await
            .map_err(|e| format!("Failed to receive prompts: {}", e))??;

        // Find the prompt that matches the command name
        let matching_prompt = prompts
            .iter()
            .find(|p| {
                // Extract the base name from the namespaced prompt name
                p.name.split("__").last().unwrap_or("") == command_name
            })
            .ok_or_else(|| format!("Slash command '{}' not found", command_name))?;

        let namespaced_name = matching_prompt.name.to_string();

        // Get the prompt content
        let (tx_get, rx_get) = oneshot::channel();
        self.mcp_tx
            .send(McpCommand::GetPrompt {
                name: namespaced_name.clone(),
                arguments,
                tx: tx_get,
            })
            .await
            .map_err(|e| format!("Failed to send GetPrompt command: {}", e))?;

        let prompt_result = rx_get
            .await
            .map_err(|e| format!("Failed to receive prompt: {}", e))??;

        // Extract text content from the first message
        if let Some(message) = prompt_result.messages.first() {
            match &message.content {
                rmcp::model::PromptMessageContent::Text { text } => Ok(text.to_string()),
                _ => Err("Prompt message does not contain text content".into()),
            }
        } else {
            Err("Prompt result contains no messages".into())
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        debug!("Dropping session {}", self.id);
        // The agent_handle and channels will be dropped, which will clean up the agent task
    }
}
