use aether::{
    agent::{AgentMessage, UserMessage, agent},
    mcp::manager::McpServerConfig,
    testing::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
};
use futures::{StreamExt, pin_mut};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_agent_cancellation() {
    // Create a fake LLM that responds slowly
    let mut fake_llm = FakeLlmProvider::new();

    // Add a response that takes time to complete
    fake_llm.add_response(vec![
        Ok(LlmResponse::Start { message_id: "msg1".to_string() }),
        Ok(LlmResponse::Text { chunk: "This is".to_string() }),
        Ok(LlmResponse::Text { chunk: " a slow".to_string() }),
        Ok(LlmResponse::Text { chunk: " response".to_string() }),
        Ok(LlmResponse::Done),
    ]);

    // Set a delay between responses to simulate slow processing
    fake_llm.set_delay(Duration::from_millis(100));

    let mut agent = agent()
        .llm(fake_llm)
        .mcp_configs(vec![])
        .build()
        .await
        .unwrap();

    // Start processing
    let stream = agent.send(UserMessage::text("Test message")).await;
    pin_mut!(stream);

    // Read a few messages
    let first_msg = stream.next().await;
    assert!(first_msg.is_some());

    let second_msg = stream.next().await;
    assert!(second_msg.is_some());

    // Verify the agent is processing
    assert!(agent.is_processing());

    // Cancel the current task
    agent.cancel_current_task();

    // Verify the agent is no longer processing
    assert!(!agent.is_processing());

    // Try to get more messages - the stream should end due to cancellation
    let remaining_messages: Vec<_> = stream.collect().await;

    // The stream should end quickly after cancellation
    // We might get a few more buffered messages, but it should stop
    println!("Remaining messages after cancellation: {}", remaining_messages.len());
}

#[tokio::test]
async fn test_agent_sequential_processing() {
    let mut fake_llm = FakeLlmProvider::new();

    // Add responses for two separate messages
    fake_llm.add_response(vec![
        Ok(LlmResponse::Start { message_id: "msg1".to_string() }),
        Ok(LlmResponse::Text { chunk: "First response".to_string() }),
        Ok(LlmResponse::Done),
    ]);

    fake_llm.add_response(vec![
        Ok(LlmResponse::Start { message_id: "msg2".to_string() }),
        Ok(LlmResponse::Text { chunk: "Second response".to_string() }),
        Ok(LlmResponse::Done),
    ]);

    let mut agent = agent()
        .llm(fake_llm)
        .mcp_configs(vec![])
        .build()
        .await
        .unwrap();

    // Send first message
    let stream1 = agent.send(UserMessage::text("First message")).await;
    pin_mut!(stream1);

    // Collect all messages from first stream
    let messages1: Vec<_> = stream1.collect().await;
    assert!(!messages1.is_empty());

    // Send second message (should cancel any remaining work from first)
    let stream2 = agent.send(UserMessage::text("Second message")).await;
    pin_mut!(stream2);

    // Collect all messages from second stream
    let messages2: Vec<_> = stream2.collect().await;
    assert!(!messages2.is_empty());

    // Verify we got responses to both messages
    println!("First message responses: {}", messages1.len());
    println!("Second message responses: {}", messages2.len());
}

#[tokio::test]
async fn test_agent_drop_cancellation() {
    let mut fake_llm = FakeLlmProvider::new();

    // Add a long-running response
    fake_llm.add_response(vec![
        Ok(LlmResponse::Start { message_id: "msg1".to_string() }),
        Ok(LlmResponse::Text { chunk: "Starting".to_string() }),
        Ok(LlmResponse::Text { chunk: " long".to_string() }),
        Ok(LlmResponse::Text { chunk: " response".to_string() }),
        Ok(LlmResponse::Done),
    ]);

    fake_llm.set_delay(Duration::from_millis(200));

    {
        let mut agent = agent()
            .llm(fake_llm)
            .mcp_configs(vec![])
            .build()
            .await
            .unwrap();

        // Start processing
        let _stream = agent.send(UserMessage::text("Test message")).await;

        // Verify processing started
        assert!(agent.is_processing());

        // Agent will be dropped here, which should cancel the task
    }

    // Brief delay to let any background tasks finish
    sleep(Duration::from_millis(10)).await;

    // If we get here without hanging, the Drop implementation worked correctly
}