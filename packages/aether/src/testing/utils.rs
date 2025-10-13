use std::error::Error;
use std::time::Duration;

use futures::future::join_all;

use crate::agent::{AgentMessage, UserMessage, agent};
use crate::mcp::mcp;
use crate::testing::FakeMcpServer;
use crate::testing::fake_mcp::fake_mcp;
use crate::types::LlmResponse;

use super::FakeLlmProvider;

pub fn test_agent() -> TestAgentBuilder {
    TestAgentBuilder::new()
}

pub struct TestAgentBuilder {
    messages: Vec<UserMessage>,
    responses: Vec<Vec<LlmResponse>>,
    timeout: Option<Duration>,
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
            .add(vec![fake_mcp("test", FakeMcpServer::new())])
            .spawn()
            .await?;

        let mut builder = agent(llm).mcp_tools(mcp_tx, tool_definitions);
        if let Some(timeout) = self.timeout {
            builder = builder.tool_timeout(timeout);
        }

        let (tx, mut rx, _handle) = builder.spawn().await?;
        let futures: Vec<_> = self.messages.iter().map(|m| tx.send(m.clone())).collect();

        join_all(futures).await;
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
