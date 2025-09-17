use aether::{
    agent::{AgentMessage, UserMessage, agent},
    testing::fake_llm::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
};
use futures::{StreamExt, pin_mut};

#[tokio::test]
async fn test_simple_tool_execution() {
    // Create a fake LLM that requests a tool call and then responds
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::Text {
            chunk: "I'll help you with that. Let me use a tool.".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "tool1".to_string(),
            name: "coding::write_file".to_string(),
        },
        LlmResponse::ToolRequestArg {
            id: "tool1".to_string(),
            chunk: r#"{"path": "/test.txt", "content": "Hello World"}"#.to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool1".to_string(),
                name: "coding::write_file".to_string(),
                arguments: r#"{"path": "/test.txt", "content": "Hello World"}"#.to_string(),
            },
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    let (stream, _cancel_token) = agent.send(UserMessage::text("Write a test file")).await;
    pin_mut!(stream);

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Verify we got the expected events
    let content_chunks: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentMessage::Text { chunk, .. } => Some(chunk.as_str()),
            _ => None,
        })
        .collect();

    let tool_calls: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentMessage::ToolCall {
                tool_call_id,
                name,
                is_complete,
                ..
            } => Some((tool_call_id.as_str(), name.as_str(), *is_complete)),
            _ => None,
        })
        .collect();

    assert!(!content_chunks.is_empty());
    assert!(!tool_calls.is_empty());

    // Check that we have at least one complete tool call
    assert!(tool_calls.iter().any(|(_, _, is_complete)| *is_complete));
}

#[tokio::test]
async fn test_tool_execution_error_handling() {
    // Create a fake LLM that makes a tool call with invalid arguments
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "tool1".to_string(),
            name: "coding::write_file".to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool1".to_string(),
                name: "coding::write_file".to_string(),
                arguments: "invalid json".to_string(), // This should cause an error
            },
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    let (stream, _cancel_token) = agent.send(UserMessage::text("Write a file")).await;
    pin_mut!(stream);

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have a tool call chunk with an error result
    let has_error_result = events.iter().any(|e| match e {
        AgentMessage::ToolCall {
            result: Some(result),
            ..
        } => result.contains("Invalid tool arguments"),
        _ => false,
    });

    assert!(
        has_error_result,
        "Expected tool execution error to be captured"
    );
}
