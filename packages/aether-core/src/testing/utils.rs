use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::join_all;

use crate::core::agent;
use crate::events::{AgentMessage, UserMessage};
use crate::mcp::McpSpawnResult;
use crate::mcp::mcp;
use crate::testing::FakeMcpServer;
use crate::testing::fake_mcp::fake_mcp;
use llm::{Context, LlmResponse};

use llm::testing::FakeLlmProvider;

pub fn test_agent() -> TestAgentBuilder {
    TestAgentBuilder::new()
}

/// Result of running a test agent, including messages and captured contexts.
pub struct TestAgentResult {
    pub messages: Vec<AgentMessage>,
    pub captured_contexts: Arc<Mutex<Vec<Context>>>,
}

pub struct TestAgentBuilder {
    messages: Vec<UserMessage>,
    responses: Vec<Vec<LlmResponse>>,
    timeout: Option<Duration>,
    max_auto_continues: Option<u32>,
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
            max_auto_continues: None,
        }
    }

    pub fn user_messages(mut self, user_messages: Vec<UserMessage>) -> Self {
        self.messages = user_messages;
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

    pub fn max_auto_continues(mut self, max: u32) -> Self {
        self.max_auto_continues = Some(max);
        self
    }

    pub async fn run(self) -> Result<Vec<AgentMessage>, Box<dyn Error>> {
        let result = self.run_with_context().await?;
        Ok(result.messages)
    }

    /// Runs the test agent and returns both messages and captured contexts.
    ///
    /// Use this when you need to verify what context was passed to the LLM,
    /// for example when testing that file attachments are properly formatted.
    pub async fn run_with_context(self) -> Result<TestAgentResult, Box<dyn Error>> {
        let llm = FakeLlmProvider::new(self.responses);
        let captured_contexts = llm.captured_contexts();

        let McpSpawnResult {
            tool_definitions,
            instructions: _,
            command_tx: mcp_tx,
            elicitation_rx: _,
            handle: _mcp_handle,
        } = mcp()
            .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
            .spawn()
            .await?;

        let mut builder = agent(llm).tools(mcp_tx, tool_definitions);
        if let Some(timeout) = self.timeout {
            builder = builder.tool_timeout(timeout);
        }
        if let Some(max) = self.max_auto_continues {
            builder = builder.max_auto_continues(max);
        }

        let (tx, mut rx, _handle) = builder.spawn().await?;
        let futures: Vec<_> = self.messages.into_iter().map(|m| tx.send(m)).collect();

        join_all(futures).await;
        drop(tx);

        let mut messages = Vec::new();
        while let Some(message) = rx.recv().await {
            messages.push(message.clone());
            if matches!(message, AgentMessage::Done) {
                break;
            }
        }

        Ok(TestAgentResult {
            messages,
            captured_contexts,
        })
    }
}
