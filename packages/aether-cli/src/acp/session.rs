use super::mappers::map_mcp_prompt_to_available_command;
use aether_core::core::AgentHandle;
use aether_core::events::{AgentMessage, UserMessage};
use aether_core::mcp::run_mcp_task::McpCommand;
use aether_project::ResolvedRuntimeSpec;
use llm::ChatMessage;
use mcp_utils::client::oauth::BrowserOAuthHandler;
use mcp_utils::client::{ElicitationRequest, McpServerConfig};
use mcp_utils::status::McpServerStatusEntry;

use agent_client_protocol as acp;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, error};

use crate::runtime::RuntimeBuilder;

/// Represents an active Aether agent session
pub struct Session {
    pub agent_tx: mpsc::Sender<UserMessage>,
    pub agent_rx: mpsc::Receiver<AgentMessage>,
    pub agent_handle: AgentHandle,
    pub _mcp_handle: JoinHandle<()>,
    pub mcp_tx: mpsc::Sender<McpCommand>,
    pub elicitation_rx: mpsc::Receiver<ElicitationRequest>,
    pub initial_server_statuses: Vec<McpServerStatusEntry>,
}

impl Session {
    /// Creates a new session with the given LLM provider and configuration.
    ///
    /// Pass `restored_messages` to pre-populate conversation history (e.g. session resume).
    pub async fn new(
        runtime: ResolvedRuntimeSpec,
        cwd: PathBuf,
        extra_mcp_servers: Vec<McpServerConfig>,
        restored_messages: Option<Vec<ChatMessage>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        debug!("MCP config: {:?}", runtime.effective_mcp_config_path);
        debug!("Using project root: {:?}", cwd);

        let mut rb = RuntimeBuilder::from_resolved(cwd, runtime)
            .extra_servers(extra_mcp_servers);

        match BrowserOAuthHandler::new() {
            Ok(handler) => {
                rb = rb.oauth_handler(handler);
            }
            Err(e) => {
                error!("Failed to initialize browser OAuth handler: {e}");
            }
        }

        let agent = rb.build(None, restored_messages).await?;

        Ok(Self {
            agent_tx: agent.agent_tx,
            agent_rx: agent.agent_rx,
            agent_handle: agent.agent_handle,
            _mcp_handle: agent.mcp_handle,
            mcp_tx: agent.mcp_tx,
            elicitation_rx: agent.elicitation_rx,
            initial_server_statuses: agent.server_statuses,
        })
    }

    /// Lists available slash commands by querying MCP prompts
    pub async fn list_available_commands(
        &self,
    ) -> Result<Vec<acp::AvailableCommand>, Box<dyn std::error::Error>> {
        let (tx, rx) = oneshot::channel();

        self.mcp_tx
            .send(McpCommand::ListPrompts { tx })
            .await
            .map_err(|e| format!("Failed to send ListPrompts command: {e}"))?;

        let prompts = rx
            .await
            .map_err(|e| format!("Failed to receive prompts: {e}"))??;

        let prompt_commands: Vec<_> = prompts
            .iter()
            .map(map_mcp_prompt_to_available_command)
            .collect();

        Ok(merge_builtin_commands(prompt_commands))
    }
}

fn merge_builtin_commands(commands: Vec<acp::AvailableCommand>) -> Vec<acp::AvailableCommand> {
    let mut merged = builtin_commands();
    let mut seen_names: HashSet<String> = merged.iter().map(|c| c.name.clone()).collect();

    for command in commands {
        if seen_names.insert(command.name.clone()) {
            merged.push(command);
        }
    }

    merged
}

fn builtin_commands() -> Vec<acp::AvailableCommand> {
    vec![acp::AvailableCommand::new(
        "clear",
        "Clear agent context and reset to a blank slate",
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_builtin_commands_includes_clear() {
        let commands = merge_builtin_commands(vec![]);
        assert!(commands.iter().any(|c| c.name == "clear"));
    }

    #[test]
    fn merge_builtin_commands_deduplicates_by_name() {
        let merged = merge_builtin_commands(vec![
            acp::AvailableCommand::new("clear", "MCP clear command"),
            acp::AvailableCommand::new("search", "Search"),
        ]);

        let clear_count = merged.iter().filter(|c| c.name == "clear").count();
        assert_eq!(clear_count, 1);
        assert!(merged.iter().any(|c| c.name == "search"));
    }
}
