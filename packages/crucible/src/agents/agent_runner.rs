use super::agent_runner_message::AgentRunnerMessage;
use std::future::Future;
use std::path::Path;
use tokio::sync::mpsc::Sender;

/// Configuration for running an agent on a specific task
pub struct AgentConfig<'a> {
    pub working_directory: &'a Path,
    pub system_prompt: Option<&'a str>,
    pub task_prompt: &'a str,
}

/// Trait for running agents on evaluation tasks
///
/// Implementors are responsible for:
/// - Creating their own MCP connections (if needed)
/// - Running the agent with the provided configuration
/// - Sending `AgentRunnerMessage`s to the provided channel
/// - Sending `AgentRunnerMessage::Done` when the agent finishes
///
/// # Example
///
/// ```ignore
/// struct MyAgentRunner;
///
/// impl AgentRunner for MyAgentRunner {
///     async fn run(&self, config: AgentConfig<'_>, tx: Sender<AgentRunnerMessage>) -> Result<(), RunError> {
///         // Create agent, run task, send messages
///         tx.send(AgentRunnerMessage::AgentText("Hello".to_string())).await
///             .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
///         tx.send(AgentRunnerMessage::Done).await
///             .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
///         Ok(())
///     }
/// }
/// ```
pub trait AgentRunner: Send + Sync {
    fn run(
        &self,
        config: AgentConfig<'_>,
        tx: Sender<AgentRunnerMessage>,
    ) -> impl Future<Output = Result<(), RunError>> + Send;
}

/// Errors that can occur when running an agent
#[derive(Debug)]
pub enum RunError {
    ExecutionFailed(String),
    ChannelSendFailed(String),
    ConfigurationError(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::ExecutionFailed(msg) => write!(f, "Agent execution failed: {msg}"),
            RunError::ChannelSendFailed(msg) => write!(f, "Failed to send event: {msg}"),
            RunError::ConfigurationError(msg) => write!(f, "Agent configuration error: {msg}"),
        }
    }
}

impl std::error::Error for RunError {}
