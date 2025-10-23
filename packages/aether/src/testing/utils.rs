use std::error::Error;
use std::time::Duration;

use crate::agent::{AgentMessage, UserMessage, agent};
use crate::llm::LlmResponse;
use crate::mcp::mcp;
use crate::testing::FakeMcpServer;
use crate::testing::fake_mcp::fake_mcp;

use super::FakeLlmProvider;

pub fn test_agent() -> TestAgentBuilder {
    TestAgentBuilder::new()
}

pub struct TestAgentBuilder {
    messages: Vec<UserMessage>,
    responses: Vec<Vec<LlmResponse>>,
    timeout: Option<Duration>,
}

impl Default for TestAgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestAgentBuilder {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            responses: Vec::new(),
            timeout: None,
        }
    }

    pub fn user_messages(mut self, user_messages: &[UserMessage]) -> Self {
        self.messages = Vec::from(user_messages);
        self
    }

    pub fn llm_responses(mut self, llm_responses: &[Vec<LlmResponse>]) -> Self {
        self.responses = Vec::from(llm_responses);
        self
    }

    pub fn tool_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub async fn run(self) -> Result<Vec<AgentMessage>, Box<dyn Error>> {
        let llm = FakeLlmProvider::new(self.responses);

        let (tool_definitions, mcp_tx, _mcp_handle) = mcp()
            .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
            .spawn()
            .await?;

        let mut builder = agent(llm).tools(mcp_tx, tool_definitions);
        if let Some(timeout) = self.timeout {
            builder = builder.tool_timeout(timeout);
        }

        let (tx, mut rx, _handle) = builder.spawn().await?;

        // Send messages sequentially without cloning (UserMessage is no longer Clone)
        for message in self.messages {
            if tx.send(message).await.is_err() {
                break;
            }
        }
        drop(tx);

        let mut messages = Vec::new();
        while let Some(message) = rx.recv().await {
            messages.push(message.clone());
            if matches!(message, AgentMessage::Done) {
                break;
            }
        }

        Ok(messages)
    }
}
