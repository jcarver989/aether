use std::error::Error;

use futures::future::join_all;

use crate::agent::{AgentMessage, UserMessage, agent};
use crate::testing::FakeMcpServer;
use crate::testing::fake_mcp::fake_mcp;
use crate::types::LlmResponse;

use super::FakeLlmProvider;

pub async fn run_agent(
    llm_responses: &[Vec<LlmResponse>],
    user_messages: &[UserMessage],
) -> Result<Vec<AgentMessage>, Box<dyn Error>> {
    let llm = FakeLlmProvider::new(Vec::from(llm_responses));

    let (tx, mut rx, _handle) = agent(llm)
        .mcp(fake_mcp("test", FakeMcpServer::new()))
        .spawn()
        .await?;

    let futures: Vec<_> = user_messages.iter().map(|m| tx.send(m.clone())).collect();
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
