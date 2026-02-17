use aether::core::{Prompt, agent};
use aether::events::{AgentMessage, UserMessage};
use aether::mcp::{McpBuilder, McpSpawnResult, mcp};
use llm::{StreamingModelProvider, ToolCallRequest};
use mcp_utils::client::ServerFactory;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};

use super::{AgentConfig, AgentRunner, AgentRunnerMessage, RunError};

/// Agent runner implementation for Aether agents
///
/// This implementation creates MCP connections for each eval run, ensuring full isolation.
/// It supports both in-memory MCP servers and external servers (via mcp.json).
///
/// # Example
///
/// ```ignore
/// use crucible::aether_runner::AetherRunner;
/// use mcp_coding::CodingMcp;
///
/// // Assume you have an LLM provider
/// let llm = /* your LLM provider */;
/// let runner = AetherRunner::new(llm)
///     .with_mcp_server_factory("coding", Box::new(|_| CodingMcp::new().into_dyn()));
/// ```
pub struct AetherRunner<T> {
    llm: T,
    factories: HashMap<String, Arc<ServerFactory>>,
    mcp_json_path: Option<PathBuf>,
}

impl<T: StreamingModelProvider + 'static> AetherRunner<T> {
    /// Create a new AetherRunner with the given LLM provider
    pub fn new(llm: T) -> Self {
        Self {
            llm,
            factories: HashMap::new(),
            mcp_json_path: None,
        }
    }

    /// Register an in-memory MCP server factory
    ///
    /// # Arguments
    /// * `name` - The name of the server (referenced in mcp.json)
    /// * `factory` - Factory function that creates server instances
    pub fn with_mcp_server_factory(
        mut self,
        name: impl Into<String>,
        factory: ServerFactory,
    ) -> Self {
        self.factories.insert(name.into(), Arc::new(factory));
        self
    }

    /// Register multiple in-memory MCP server factories
    pub fn with_mcp_server_factories(mut self, factories: HashMap<String, ServerFactory>) -> Self {
        for (name, factory) in factories {
            self.factories.insert(name, Arc::new(factory));
        }
        self
    }

    /// Set the path to mcp.json for external MCP servers
    pub fn with_mcp_json(mut self, path: impl Into<PathBuf>) -> Self {
        self.mcp_json_path = Some(path.into());
        self
    }

    async fn create_mcp_builder(&self) -> Result<McpBuilder, RunError> {
        let mut mcp_builder = mcp();

        // Clone the Arc, not the factory itself
        for (name, factory) in &self.factories {
            // We need to create a new factory from the Arc by cloning the Arc and calling it
            let factory_arc = factory.clone();
            let factory_fn: ServerFactory = Box::new(move |args| factory_arc(args));
            mcp_builder = mcp_builder.register_in_memory_server(name.clone(), factory_fn);
        }

        if let Some(mcp_json_path) = &self.mcp_json_path {
            mcp_builder = mcp_builder
                .from_json_file(mcp_json_path.to_str().ok_or_else(|| {
                    RunError::ConfigurationError("Invalid mcp.json path".to_string())
                })?)
                .await
                .map_err(|e| {
                    RunError::ConfigurationError(format!("Failed to load mcp.json: {}", e))
                })?;
        }

        Ok(mcp_builder)
    }
}

/// Convert AgentMessages to AgentRunnerMessages in real-time, streaming them as they arrive
async fn stream_agent_messages(
    mut rx: Receiver<AgentMessage>,
    tx: Sender<AgentRunnerMessage>,
) -> Result<(), RunError> {
    let mut accumulated_text = String::new();
    let mut accumulated_tool_calls: HashMap<String, ToolCallRequest> = HashMap::new();

    while let Some(message) = rx.recv().await {
        match &message {
            AgentMessage::Text {
                chunk, is_complete, ..
            } => {
                accumulated_text.push_str(chunk);
                if *is_complete && !accumulated_text.is_empty() {
                    // Log each line separately to make grep work better
                    for line in accumulated_text.lines() {
                        tracing::debug!("Agent response: {}", line);
                    }
                    tx.send(AgentRunnerMessage::AgentText(accumulated_text.clone()))
                        .await
                        .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
                    accumulated_text.clear();
                }
            }
            AgentMessage::ToolCall { request, .. } => {
                let entry = accumulated_tool_calls
                    .entry(request.id.clone())
                    .or_insert_with(|| ToolCallRequest {
                        id: request.id.clone(),
                        name: String::new(),
                        arguments: String::new(),
                    });

                // Accumulate tool call data
                if !request.name.is_empty() {
                    entry.name.push_str(&request.name);
                }
                entry.arguments.push_str(&request.arguments);

                // Check if this is a complete tool call
                if !entry.name.is_empty() && entry.arguments.ends_with('}') {
                    tracing::debug!("Tool call: {} with args: {}", entry.name, entry.arguments);
                    tx.send(AgentRunnerMessage::ToolCall {
                        name: entry.name.clone(),
                        arguments: entry.arguments.clone(),
                    })
                    .await
                    .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
                    accumulated_tool_calls.remove(&request.id);
                }
            }
            AgentMessage::ToolResult { result, .. } => {
                tracing::debug!("Tool result for {}: {}", result.name, result.result);
                tx.send(AgentRunnerMessage::ToolResult {
                    name: result.name.clone(),
                    result: result.result.clone(),
                })
                .await
                .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
            }
            AgentMessage::ToolError { error, .. } => {
                tracing::debug!("Tool error: {:?}", error);
                tx.send(AgentRunnerMessage::ToolError(format!("{error:?}")))
                    .await
                    .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
            }
            AgentMessage::ToolProgress {
                request,
                progress,
                total,
                message,
            } => {
                let msg = message
                    .as_ref()
                    .map(|m| format!("{m} "))
                    .unwrap_or_default();
                let total_str = total.map(|t| format!("/{t}")).unwrap_or_default();
                tracing::debug!(
                    "Tool progress for {}: {}{}{}",
                    request.name,
                    msg,
                    progress,
                    total_str
                );
                // Progress events don't need to be captured in eval messages
            }
            AgentMessage::Error { message: msg } => {
                tracing::debug!("Agent error: {}", msg);
                tx.send(AgentRunnerMessage::Error(msg.clone()))
                    .await
                    .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
                // Agent errors are terminal - agent won't send Done, so break out
                break;
            }
            AgentMessage::Cancelled { message: msg } => {
                tracing::debug!("Agent cancelled: {}", msg);
                tx.send(AgentRunnerMessage::Error(format!("Cancelled: {msg}")))
                    .await
                    .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
                // Cancellation is terminal - break out
                break;
            }
            AgentMessage::Done => {
                // Log any remaining accumulated text before finishing
                if !accumulated_text.is_empty() {
                    for line in accumulated_text.lines() {
                        tracing::debug!("Agent response: {}", line);
                    }
                    tx.send(AgentRunnerMessage::AgentText(accumulated_text.clone()))
                        .await
                        .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
                    accumulated_text.clear();
                }
                tracing::debug!("Agent done");
                tx.send(AgentRunnerMessage::Done)
                    .await
                    .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
                break;
            }
            AgentMessage::ContextCompactionStarted { message_count } => {
                tracing::debug!("Context compaction started: {} messages", message_count);
            }
            AgentMessage::ContextCompactionResult {
                messages_removed, ..
            } => {
                tracing::debug!("Context compacted: {} messages removed", messages_removed);
            }
            AgentMessage::ContextUsageUpdate {
                usage_ratio,
                tokens_used,
                context_limit,
            } => {
                tracing::debug!(
                    "Context usage: {:.1}% ({}/{} tokens)",
                    usage_ratio * 100.0,
                    tokens_used,
                    context_limit
                );
            }
            AgentMessage::AutoContinue {
                attempt,
                max_attempts,
            } => {
                tracing::debug!(
                    "Auto-continuing: attempt {}/{} - LLM stopped without completion signal",
                    attempt,
                    max_attempts
                );
            }
            AgentMessage::ModelSwitched { previous, new } => {
                tracing::debug!("Model switched: {} -> {}", previous, new);
            }
            AgentMessage::Thought { chunk, .. } => {
                tracing::debug!("Agent thought: {}", chunk);
            }
        }
    }

    Ok(())
}

impl<T: StreamingModelProvider + Clone + 'static> AgentRunner for AetherRunner<T> {
    async fn run(
        &self,
        config: AgentConfig<'_>,
        tx: Sender<AgentRunnerMessage>,
    ) -> Result<(), RunError> {
        let mcp_builder = self.create_mcp_builder().await?;
        let McpSpawnResult {
            tool_definitions,
            instructions,
            command_tx,
            handle: _mcp_handle,
        } = mcp_builder
            .spawn()
            .await
            .map_err(|e| RunError::ExecutionFailed(format!("Failed to spawn MCP: {}", e)))?;

        let llm = self.llm.clone();
        let mut agent_builder = agent(llm)
            .system_prompt(Prompt::mcp_instructions(instructions))
            .tools(command_tx, tool_definitions);

        if let Some(prompt) = config.system_prompt {
            agent_builder = agent_builder.system_prompt(Prompt::text(prompt));
        }

        let (agent_tx, agent_rx, _handle) = agent_builder
            .spawn()
            .await
            .map_err(|e| RunError::ExecutionFailed(format!("Failed to spawn agent: {}", e)))?;

        agent_tx
            .send(UserMessage::text(config.task_prompt))
            .await
            .map_err(|e| RunError::ChannelSendFailed(format!("Failed to send task: {e}")))?;

        stream_agent_messages(agent_rx, tx).await
    }
}
