use aether_core::core::{Prompt, agent};
use aether_core::events::{AgentMessage, UserMessage};
use aether_core::mcp::{McpBuilder, McpSpawnResult, mcp};
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
    /// Create a new `AetherRunner` with the given LLM provider
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
            let factory_fn: ServerFactory = Box::new(move |args, input| factory_arc(args, input));
            mcp_builder = mcp_builder.register_in_memory_server(name.clone(), factory_fn);
        }

        if let Some(mcp_json_path) = &self.mcp_json_path {
            mcp_builder = mcp_builder
                .from_json_file(mcp_json_path.to_str().ok_or_else(|| {
                    RunError::ConfigurationError("Invalid mcp.json path".to_string())
                })?)
                .await
                .map_err(|e| {
                    RunError::ConfigurationError(format!("Failed to load mcp.json: {e}"))
                })?;
        }

        Ok(mcp_builder)
    }
}

/// Convert `AgentMessages` to `AgentRunnerMessages` in real-time, streaming them as they arrive
#[allow(clippy::too_many_lines)]
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
            } => handle_text(chunk, *is_complete, &mut accumulated_text, &tx).await?,

            AgentMessage::ToolCall { request, .. } => {
                handle_tool_call(request, &mut accumulated_tool_calls);
            }

            AgentMessage::ToolCallUpdate {
                tool_call_id,
                chunk,
                ..
            } => {
                handle_tool_call_update(tool_call_id, chunk, &mut accumulated_tool_calls);
            }

            AgentMessage::ToolResult { result, .. } => {
                if let Some(tool_call) = take_tool_call(
                    &mut accumulated_tool_calls,
                    &result.id,
                    &result.name,
                    &result.arguments,
                ) {
                    tx.send(tool_call)
                        .await
                        .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
                }
                tracing::debug!("Tool result for {}: {}", result.name, result.result);
                tx.send(AgentRunnerMessage::ToolResult {
                    name: result.name.clone(),
                    result: result.result.clone(),
                })
                .await
                .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
            }

            AgentMessage::ToolError { error, .. } => {
                if let Some(tool_call) = take_tool_call(
                    &mut accumulated_tool_calls,
                    &error.id,
                    &error.name,
                    error.arguments.as_deref().unwrap_or_default(),
                ) {
                    tx.send(tool_call)
                        .await
                        .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
                }
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
            } => handle_tool_progress(request, *progress, *total, message.as_ref()),

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
                handle_done(&mut accumulated_text, &tx).await?;
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
            } => match (usage_ratio, context_limit) {
                (Some(usage_ratio), Some(context_limit)) => {
                    tracing::debug!(
                        "Context usage: {:.1}% ({}/{} tokens)",
                        usage_ratio * 100.0,
                        tokens_used,
                        context_limit
                    );
                }
                _ => {
                    tracing::debug!("Context usage: unknown limit ({} tokens used)", tokens_used);
                }
            },
            AgentMessage::AutoContinue {
                attempt,
                max_attempts,
            } => {
                tracing::debug!(
                    "Auto-continuing: attempt {}/{} - LLM stopped with resumable stop reason",
                    attempt,
                    max_attempts
                );
            }
            AgentMessage::ModelSwitched { previous, new } => {
                tracing::debug!("Model switched: {} -> {}", previous, new);
            }
            AgentMessage::ContextCleared => {
                tracing::debug!("Agent context cleared");
            }
            AgentMessage::Thought {
                chunk,
                is_complete: false,
                ..
            } => {
                tracing::debug!("Agent thought: {}", chunk);
            }
            AgentMessage::Thought {
                is_complete: true, ..
            } => {}
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
            elicitation_rx: _,
            handle: _mcp_handle,
            server_statuses: _,
        } = mcp_builder
            .spawn()
            .await
            .map_err(|e| RunError::ExecutionFailed(format!("Failed to spawn MCP: {e}")))?;

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
            .map_err(|e| RunError::ExecutionFailed(format!("Failed to spawn agent: {e}")))?;

        agent_tx
            .send(UserMessage::text(config.task_prompt))
            .await
            .map_err(|e| RunError::ChannelSendFailed(format!("Failed to send task: {e}")))?;

        stream_agent_messages(agent_rx, tx).await
    }
}

async fn handle_text(
    chunk: &str,
    is_complete: bool,
    accumulated_text: &mut String,
    tx: &Sender<AgentRunnerMessage>,
) -> Result<(), RunError> {
    accumulated_text.push_str(chunk);
    if is_complete && !accumulated_text.is_empty() {
        for line in accumulated_text.lines() {
            tracing::debug!("Agent response: {}", line);
        }
        tx.send(AgentRunnerMessage::AgentText(accumulated_text.clone()))
            .await
            .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
        accumulated_text.clear();
    }
    Ok(())
}

fn upsert_tool_call<'a>(
    accumulated: &'a mut HashMap<String, ToolCallRequest>,
    id: &str,
    name: Option<&str>,
    arguments: Option<&str>,
) -> &'a mut ToolCallRequest {
    let entry = accumulated
        .entry(id.to_string())
        .or_insert_with(|| ToolCallRequest {
            id: id.to_string(),
            name: String::new(),
            arguments: String::new(),
        });

    if let Some(name) = name.filter(|n| !n.is_empty()) {
        entry.name = name.to_string();
    }
    if let Some(args) = arguments {
        entry.arguments.push_str(args);
    }
    entry
}

fn handle_tool_call(
    request: &ToolCallRequest,
    accumulated_tool_calls: &mut HashMap<String, ToolCallRequest>,
) {
    let name = (!request.name.is_empty()).then_some(request.name.as_str());
    let args = (!request.arguments.is_empty()).then_some(request.arguments.as_str());
    upsert_tool_call(accumulated_tool_calls, &request.id, name, args);
}

fn handle_tool_call_update(
    tool_call_id: &str,
    chunk: &str,
    accumulated_tool_calls: &mut HashMap<String, ToolCallRequest>,
) {
    upsert_tool_call(accumulated_tool_calls, tool_call_id, None, Some(chunk));
}

fn take_tool_call(
    accumulated_tool_calls: &mut HashMap<String, ToolCallRequest>,
    tool_call_id: &str,
    fallback_name: &str,
    fallback_arguments: &str,
) -> Option<AgentRunnerMessage> {
    let request = accumulated_tool_calls.remove(tool_call_id)?;
    let name = if request.name.is_empty() {
        fallback_name.to_string()
    } else {
        request.name
    };
    let arguments = if request.arguments.is_empty() {
        fallback_arguments.to_string()
    } else {
        request.arguments
    };

    Some(AgentRunnerMessage::ToolCall { name, arguments })
}

fn handle_tool_progress(
    request: &ToolCallRequest,
    progress: f64,
    total: Option<f64>,
    message: Option<&String>,
) {
    let msg = message.map(|m| format!("{m} ")).unwrap_or_default();
    let total_str = total.map(|t| format!("/{t}")).unwrap_or_default();
    tracing::debug!(
        "Tool progress for {}: {}{}{}",
        request.name,
        msg,
        progress,
        total_str
    );
}

async fn handle_done(
    accumulated_text: &mut String,
    tx: &Sender<AgentRunnerMessage>,
) -> Result<(), RunError> {
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm::{ToolCallError, ToolCallResult};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn stream_agent_messages_emits_tool_call_when_result_arrives() {
        let (agent_tx, agent_rx) = mpsc::channel(8);
        let (runner_tx, mut runner_rx) = mpsc::channel(8);

        let task = tokio::spawn(async move { stream_agent_messages(agent_rx, runner_tx).await });

        agent_tx
            .send(AgentMessage::ToolCall {
                request: ToolCallRequest {
                    id: "call_1".to_string(),
                    name: "coding__read_file".to_string(),
                    arguments: String::new(),
                },
                model_name: "test".to_string(),
            })
            .await
            .unwrap();
        agent_tx
            .send(AgentMessage::ToolCallUpdate {
                tool_call_id: "call_1".to_string(),
                chunk: r#"["Cargo.toml"]"#.to_string(),
                model_name: "test".to_string(),
            })
            .await
            .unwrap();
        agent_tx
            .send(AgentMessage::ToolResult {
                result: ToolCallResult {
                    id: "call_1".to_string(),
                    name: "coding__read_file".to_string(),
                    arguments: r#"["Cargo.toml"]"#.to_string(),
                    result: "file contents".to_string(),
                },
                result_meta: None,
                model_name: "test".to_string(),
            })
            .await
            .unwrap();
        agent_tx.send(AgentMessage::Done).await.unwrap();
        drop(agent_tx);

        let mut messages = Vec::new();
        while let Some(message) = runner_rx.recv().await {
            let is_done = matches!(message, AgentRunnerMessage::Done);
            messages.push(message);
            if is_done {
                break;
            }
        }

        task.await.unwrap().unwrap();

        assert!(matches!(
            &messages[0],
            AgentRunnerMessage::ToolCall { name, arguments }
                if name == "coding__read_file" && arguments == r#"["Cargo.toml"]"#
        ));
        assert!(matches!(
            &messages[1],
            AgentRunnerMessage::ToolResult { name, result }
                if name == "coding__read_file" && result == "file contents"
        ));
        assert!(matches!(messages.last(), Some(AgentRunnerMessage::Done)));
    }

    #[tokio::test]
    async fn stream_agent_messages_emits_tool_call_when_error_arrives() {
        let (agent_tx, agent_rx) = mpsc::channel(8);
        let (runner_tx, mut runner_rx) = mpsc::channel(8);

        let task = tokio::spawn(async move { stream_agent_messages(agent_rx, runner_tx).await });

        agent_tx
            .send(AgentMessage::ToolCall {
                request: ToolCallRequest {
                    id: "call_1".to_string(),
                    name: "coding__read_file".to_string(),
                    arguments: String::new(),
                },
                model_name: "test".to_string(),
            })
            .await
            .unwrap();
        agent_tx
            .send(AgentMessage::ToolCallUpdate {
                tool_call_id: "call_1".to_string(),
                chunk: r#"["Cargo.toml"]"#.to_string(),
                model_name: "test".to_string(),
            })
            .await
            .unwrap();
        agent_tx
            .send(AgentMessage::ToolError {
                error: ToolCallError {
                    id: "call_1".to_string(),
                    name: "coding__read_file".to_string(),
                    arguments: Some(r#"["Cargo.toml"]"#.to_string()),
                    error: "boom".to_string(),
                },
                model_name: "test".to_string(),
            })
            .await
            .unwrap();
        agent_tx.send(AgentMessage::Done).await.unwrap();
        drop(agent_tx);

        let mut messages = Vec::new();
        while let Some(message) = runner_rx.recv().await {
            let is_done = matches!(message, AgentRunnerMessage::Done);
            messages.push(message);
            if is_done {
                break;
            }
        }

        task.await.unwrap().unwrap();

        assert!(matches!(
            &messages[0],
            AgentRunnerMessage::ToolCall { name, arguments }
                if name == "coding__read_file" && arguments == r#"["Cargo.toml"]"#
        ));
        assert!(
            matches!(&messages[1], AgentRunnerMessage::ToolError(error) if error.contains("boom"))
        );
        assert!(matches!(messages.last(), Some(AgentRunnerMessage::Done)));
    }
}
