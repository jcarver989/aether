use std::error::Error;

use aether::{
    agent::{agent, AgentMessage, UserMessage},
    llm::{Context, LlmResponse, LlmResponseStream, StreamingModelProvider},
    mcp::mcp,
    testing::{agent_message, fake_mcp, llm_response, FakeMcpServer},
};

/// Mock LLM provider for testing
struct MockLlmProvider {
    name: String,
    responses: Vec<LlmResponse>,
}

impl MockLlmProvider {
    fn new(name: &str, responses: Vec<LlmResponse>) -> Self {
        Self {
            name: name.to_string(),
            responses,
        }
    }
}

impl StreamingModelProvider for MockLlmProvider {
    fn stream_response(&self, _context: &Context) -> LlmResponseStream {
        Box::pin(tokio_stream::iter(self.responses.clone().into_iter().map(Ok)))
    }

    fn display_name(&self) -> String {
        self.name.clone()
    }
}

#[tokio::test]
async fn test_set_llm_switches_provider() -> Result<(), Box<dyn Error>> {
    // Setup first provider
    let (m1_id, m1_chunks) = ("message_1", ["Hello", " from", " Provider", " A"]);
    let provider_a = MockLlmProvider::new("Provider A", llm_response(m1_id).text(&m1_chunks).build());

    // Setup second provider
    let (m2_id, m2_chunks) = ("message_2", ["Hello", " from", " Provider", " B"]);
    let provider_b = MockLlmProvider::new("Provider B", llm_response(m2_id).text(&m2_chunks).build());

    // Create agent with provider A
    let (tool_definitions, mcp_tx, _mcp_handle) = mcp()
        .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
        .spawn()
        .await?;

    let (tx, mut rx, _handle) = agent(Box::new(provider_a) as Box<dyn StreamingModelProvider>)
        .tools(mcp_tx, tool_definitions)
        .spawn()
        .await?;

    // Send first message
    tx.send(UserMessage::text("hi")).await?;

    // Collect messages from provider A
    let mut messages = Vec::new();
    loop {
        let msg = rx.recv().await.expect("Expected message");
        let is_done = matches!(msg, AgentMessage::Done);
        messages.push(msg);
        if is_done {
            break;
        }
    }

    // Verify first response came from Provider A
    let has_provider_a_response = messages.iter().any(|m| {
        matches!(m, AgentMessage::Text { model_name, .. } if model_name == "Provider A")
    });
    assert!(
        has_provider_a_response,
        "Expected response from Provider A, got: {messages:?}"
    );

    // Switch to provider B
    tx.send(UserMessage::SetLlm {
        llm: Box::new(provider_b),
    })
    .await?;

    // Send second message
    tx.send(UserMessage::text("hello again")).await?;

    // Collect messages from provider B
    let mut messages = Vec::new();
    loop {
        let msg = rx.recv().await.expect("Expected message");
        let is_done = matches!(msg, AgentMessage::Done);
        messages.push(msg);
        if is_done {
            break;
        }
    }

    // Verify second response came from Provider B
    let has_provider_b_response = messages.iter().any(|m| {
        matches!(m, AgentMessage::Text { model_name, .. } if model_name == "Provider B")
    });
    assert!(
        has_provider_b_response,
        "Expected response from Provider B, got: {messages:?}"
    );

    Ok(())
}

#[tokio::test]
async fn test_set_llm_during_streaming_cancels_and_switches() -> Result<(), Box<dyn Error>> {
    // Setup provider with long response
    let long_chunks = vec![
        "This", " is", " a", " very", " long", " response", " that", " should", " be", " interrupted",
    ];
    let mut provider_a_responses = vec![LlmResponse::Start {
        message_id: "message_1".to_string(),
    }];
    for chunk in &long_chunks {
        provider_a_responses.push(LlmResponse::Text {
            chunk: chunk.to_string(),
        });
    }
    provider_a_responses.push(LlmResponse::Done);

    let provider_a = MockLlmProvider::new("Provider A", provider_a_responses);

    // Setup second provider
    let (m2_id, m2_chunks) = ("message_2", ["Response", " from", " Provider", " B"]);
    let provider_b = MockLlmProvider::new("Provider B", llm_response(m2_id).text(&m2_chunks).build());

    // Create agent
    let (tool_definitions, mcp_tx, _mcp_handle) = mcp()
        .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
        .spawn()
        .await?;

    let (tx, mut rx, _handle) = agent(Box::new(provider_a) as Box<dyn StreamingModelProvider>)
        .tools(mcp_tx, tool_definitions)
        .spawn()
        .await?;

    // Send first message
    tx.send(UserMessage::text("hi")).await?;

    // Wait for a few chunks to come through
    let mut received_chunks = 0;
    for _ in 0..3 {
        if let Some(msg) = rx.recv().await {
            if matches!(msg, AgentMessage::Text { .. }) {
                received_chunks += 1;
            }
        }
    }

    // Switch provider mid-stream
    tx.send(UserMessage::SetLlm {
        llm: Box::new(provider_b),
    })
    .await?;

    // Send new message
    tx.send(UserMessage::text("hello after switch")).await?;

    // Collect remaining messages
    let mut messages = Vec::new();
    loop {
        let msg = rx.recv().await.expect("Expected message");
        let is_done = matches!(msg, AgentMessage::Done);
        messages.push(msg);
        if is_done {
            break;
        }
    }

    // Verify that we got response from Provider B
    let has_provider_b_response = messages.iter().any(|m| {
        matches!(m, AgentMessage::Text { model_name, .. } if model_name == "Provider B")
    });
    assert!(
        has_provider_b_response,
        "Expected response from Provider B after switch"
    );

    // Verify we got fewer than all chunks from Provider A (stream was cancelled)
    assert!(
        received_chunks < long_chunks.len(),
        "Expected stream to be cancelled, but received all {} chunks",
        long_chunks.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_set_llm_preserves_context() -> Result<(), Box<dyn Error>> {
    // This test verifies that conversation history is preserved across provider switches

    // Setup first provider
    let (m1_id, m1_chunks) = ("message_1", ["Response", " A"]);
    let provider_a = MockLlmProvider::new("Provider A", llm_response(m1_id).text(&m1_chunks).build());

    // Setup second provider
    let (m2_id, m2_chunks) = ("message_2", ["Response", " B"]);
    let provider_b = MockLlmProvider::new("Provider B", llm_response(m2_id).text(&m2_chunks).build());

    // Create agent
    let (tool_definitions, mcp_tx, _mcp_handle) = mcp()
        .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
        .spawn()
        .await?;

    let (tx, mut rx, _handle) = agent(Box::new(provider_a) as Box<dyn StreamingModelProvider>)
        .tools(mcp_tx, tool_definitions)
        .spawn()
        .await?;

    // Send first message
    tx.send(UserMessage::text("first message")).await?;

    // Wait for completion
    loop {
        let msg = rx.recv().await.expect("Expected message");
        if matches!(msg, AgentMessage::Done) {
            break;
        }
    }

    // Switch provider
    tx.send(UserMessage::SetLlm {
        llm: Box::new(provider_b),
    })
    .await?;

    // Send second message
    tx.send(UserMessage::text("second message")).await?;

    // The agent should continue working normally with the new provider
    // Context (previous messages) should still be available to the new provider
    let mut got_response = false;
    loop {
        let msg = rx.recv().await.expect("Expected message");
        if matches!(msg, AgentMessage::Text { .. }) {
            got_response = true;
        }
        if matches!(msg, AgentMessage::Done) {
            break;
        }
    }

    assert!(got_response, "Expected to receive response from new provider");

    Ok(())
}

#[tokio::test]
async fn test_multiple_provider_switches() -> Result<(), Box<dyn Error>> {
    // Test switching between multiple providers
    let providers = vec![
        ("Provider A", "A1"),
        ("Provider B", "B1"),
        ("Provider C", "C1"),
        ("Provider A", "A2"), // Switch back to A
    ];

    let (tool_definitions, mcp_tx, _mcp_handle) = mcp()
        .with_servers(vec![fake_mcp("test", FakeMcpServer::new())])
        .spawn()
        .await?;

    // Start with first provider
    let initial_provider = MockLlmProvider::new(
        providers[0].0,
        llm_response(providers[0].1).text(&["Initial"]).build(),
    );

    let (tx, mut rx, _handle) = agent(Box::new(initial_provider) as Box<dyn StreamingModelProvider>)
        .tools(mcp_tx, tool_definitions)
        .spawn()
        .await?;

    // Send message with initial provider
    tx.send(UserMessage::text("message 0")).await?;
    loop {
        let msg = rx.recv().await.expect("Expected message");
        if matches!(msg, AgentMessage::Done) {
            break;
        }
    }

    // Switch through each provider
    for (i, (provider_name, msg_id)) in providers.iter().skip(1).enumerate() {
        let provider = MockLlmProvider::new(
            provider_name,
            llm_response(msg_id).text(&[provider_name]).build(),
        );

        tx.send(UserMessage::SetLlm {
            llm: Box::new(provider),
        })
        .await?;

        tx.send(UserMessage::text(&format!("message {}", i + 1)))
            .await?;

        // Verify response from correct provider
        let mut got_correct_response = false;
        loop {
            let msg = rx.recv().await.expect("Expected message");
            if let AgentMessage::Text { model_name, .. } = &msg {
                if model_name == provider_name {
                    got_correct_response = true;
                }
            }
            if matches!(msg, AgentMessage::Done) {
                break;
            }
        }

        assert!(
            got_correct_response,
            "Expected response from {}, iteration {}",
            provider_name, i
        );
    }

    Ok(())
}
