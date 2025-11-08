use super::agent_runner::{AgentConfig, AgentRunner, RunError};
use super::agent_runner_message::AgentRunnerMessage;
use tokio::sync::mpsc::Sender;

/// Fake agent runner for testing
///
/// This implementation returns predefined messages instead of running a real agent.
/// Useful for testing the eval framework without needing actual LLM providers.
///
/// # Example
///
/// ```
/// use crucible::{AgentRunnerMessage, FakeAgentRunner};
///
/// let messages = vec![
///     AgentRunnerMessage::AgentText("I'll create the file".to_string()),
///     AgentRunnerMessage::Done,
/// ];
/// let runner = FakeAgentRunner::new(messages);
/// ```
#[derive(Clone)]
pub struct FakeAgentRunner {
    messages: Vec<AgentRunnerMessage>,
}

impl FakeAgentRunner {
    /// Create a new FakeAgentRunner that will return the given messages
    pub fn new(messages: Vec<AgentRunnerMessage>) -> Self {
        Self { messages }
    }

    /// Create a FakeAgentRunner that returns a simple success response
    pub fn success() -> Self {
        Self::new(vec![
            AgentRunnerMessage::AgentText("Task completed successfully".to_string()),
            AgentRunnerMessage::Done,
        ])
    }

    /// Create a FakeAgentRunner that simulates tool usage
    pub fn with_tool_call(tool_name: impl Into<String>, result: impl Into<String>) -> Self {
        let tool_name = tool_name.into();
        Self::new(vec![
            AgentRunnerMessage::ToolCall {
                name: tool_name.clone(),
                arguments: "{}".to_string(),
            },
            AgentRunnerMessage::ToolResult {
                name: tool_name,
                result: result.into(),
            },
            AgentRunnerMessage::AgentText("Task completed using tools".to_string()),
            AgentRunnerMessage::Done,
        ])
    }
}

impl AgentRunner for FakeAgentRunner {
    async fn run(
        &self,
        _config: AgentConfig<'_>,
        tx: Sender<AgentRunnerMessage>,
    ) -> Result<(), RunError> {
        // Send all predefined messages
        for message in &self.messages {
            tx.send(message.clone())
                .await
                .map_err(|e| RunError::ChannelSendFailed(e.to_string()))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fake_runner_success() {
        let runner = FakeAgentRunner::success();
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        let config = AgentConfig {
            working_directory: std::path::Path::new("/tmp"),
            system_prompt: None,
            task_prompt: "test task",
        };

        let result = runner.run(config, tx).await;
        assert!(result.is_ok());

        // Should receive: AgentText message, Done message
        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, AgentRunnerMessage::AgentText(_)));

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, AgentRunnerMessage::Done));
    }

    #[tokio::test]
    async fn test_fake_runner_with_tool_call() {
        let runner = FakeAgentRunner::with_tool_call("bash", "success");
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        let config = AgentConfig {
            working_directory: std::path::Path::new("/tmp"),
            system_prompt: None,
            task_prompt: "test task",
        };

        let result = runner.run(config, tx).await;
        assert!(result.is_ok());

        // Should receive 4 messages (ToolCall, ToolResult, AgentText, Done)
        let mut count = 0;
        while let Some(msg) = rx.recv().await {
            count += 1;
            if matches!(msg, AgentRunnerMessage::Done) {
                break;
            }
        }
        assert_eq!(count, 4);
    }
}
