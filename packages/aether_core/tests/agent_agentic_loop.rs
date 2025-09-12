use aether_core::{
    agent::{Agent, AgentMessage},
    mcp::McpManager,
    testing::fake_llm::FakeLlmProvider,
    types::{ChatMessage, LlmMessage, ToolCallRequest},
};
use futures::pin_mut;
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_simple_tool_execution() {
    // Create a fake LLM that requests a tool call and then responds
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmMessage::Start {
            message_id: "msg1".to_string(),
        },
        LlmMessage::Message {
            chunk: "I'll help you with that. Let me use a tool.".to_string(),
        },
        LlmMessage::ToolRequestStart {
            id: "tool1".to_string(),
            name: "test_server::calculator".to_string(),
        },
        LlmMessage::ToolRequestArg {
            id: "tool1".to_string(),
            chunk: r#"{"operation": "add", "a": 5, "b": 3}"#.to_string(),
        },
        LlmMessage::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool1".to_string(),
                name: "test_server::calculator".to_string(),
                arguments: r#"{"operation": "add", "a": 5, "b": 3}"#.to_string(),
            },
        },
        LlmMessage::Done,
    ]);

    let mut agent = Agent::new(fake_llm, Some("You are a helpful assistant.".to_string()));

    let stream = agent.send_message("Calculate 5 + 3").await;
    pin_mut!(stream);

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Verify we got the expected events
    let content_chunks: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentMessage::MessageChunk { chunk, .. } => Some(chunk.as_str()),
            _ => None,
        })
        .collect();

    let tool_calls: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentMessage::ToolCallChunk {
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
async fn test_recursive_tool_calls() {
    // Create a fake LLM that makes multiple rounds of tool calls
    let fake_llm = FakeLlmProvider::new(vec![
        // First LLM response with a tool call
        vec![
            LlmMessage::Start {
                message_id: "msg1".to_string(),
            },
            LlmMessage::Message {
                chunk: "I need to gather some information first.".to_string(),
            },
            LlmMessage::ToolRequestStart {
                id: "tool1".to_string(),
                name: "test_server::get_data".to_string(),
            },
            LlmMessage::ToolRequestComplete {
                tool_call: ToolCallRequest {
                    id: "tool1".to_string(),
                    name: "test_server::get_data".to_string(),
                    arguments: r#"{"query": "user_info"}"#.to_string(),
                },
            },
            LlmMessage::Done,
        ],
        // Second LLM response with another tool call based on the first result
        vec![
            LlmMessage::Start {
                message_id: "msg2".to_string(),
            },
            LlmMessage::Message {
                chunk: "Now I'll process that information.".to_string(),
            },
            LlmMessage::ToolRequestStart {
                id: "tool2".to_string(),
                name: "test_server::process_data".to_string(),
            },
            LlmMessage::ToolRequestComplete {
                tool_call: ToolCallRequest {
                    id: "tool2".to_string(),
                    name: "test_server::process_data".to_string(),
                    arguments: r#"{"data": "processed"}"#.to_string(),
                },
            },
            LlmMessage::Done,
        ],
        // Final LLM response with no tool calls (completion)
        vec![
            LlmMessage::Start {
                message_id: "msg3".to_string(),
            },
            LlmMessage::Message {
                chunk: "Here's your final result based on the processed data.".to_string(),
            },
            LlmMessage::Done,
        ],
    ]);

    let mut agent = Agent::new(fake_llm, Some("You are a helpful assistant.".to_string()));

    let stream = agent.send_message("Process my data").await;
    pin_mut!(stream);

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Count the number of complete message chunks (indicates LLM calls)
    let complete_messages = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                AgentMessage::MessageChunk {
                    is_complete: true,
                    ..
                }
            )
        })
        .count();

    // Should have 3 complete messages (3 rounds of LLM calls)
    assert_eq!(complete_messages, 3);

    // Count tool call completions
    let completed_tool_calls = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                AgentMessage::ToolCallChunk {
                    is_complete: true,
                    ..
                }
            )
        })
        .count();

    // Should have 2 completed tool calls
    assert_eq!(completed_tool_calls, 2);
}

#[tokio::test]
async fn test_max_recursion_depth() {
    // Create a fake LLM that keeps making tool calls forever
    let tool_call_response = vec![
        LlmMessage::Start {
            message_id: "msg".to_string(),
        },
        LlmMessage::ToolRequestStart {
            id: "tool".to_string(),
            name: "test_server::endless_tool".to_string(),
        },
        LlmMessage::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool".to_string(),
                name: "test_server::endless_tool".to_string(),
                arguments: "{}".to_string(),
            },
        },
        LlmMessage::Done,
    ];

    // Create responses that repeat the tool call pattern
    let mut responses = Vec::new();
    for _ in 0..15 {
        // More than MAX_RECURSION_DEPTH (10)
        responses.push(tool_call_response.clone());
    }

    let fake_llm = FakeLlmProvider::new(responses);
    let mut agent = Agent::new(fake_llm, Some("You are a helpful assistant.".to_string()));

    let stream = agent.send_message("Start endless loop").await;
    pin_mut!(stream);

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should eventually get an error about max recursion depth
    let has_recursion_error = events
        .iter()
        .any(|e| matches!(e, AgentMessage::Error { message } if message.contains("Maximum recursion depth")));

    assert!(has_recursion_error, "Expected recursion depth error");
}

#[tokio::test]
async fn test_tool_execution_error_handling() {
    // Create a fake LLM that makes a tool call with invalid arguments
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmMessage::Start {
            message_id: "msg1".to_string(),
        },
        LlmMessage::ToolRequestStart {
            id: "tool1".to_string(),
            name: "test_server::calculator".to_string(),
        },
        LlmMessage::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool1".to_string(),
                name: "test_server::calculator".to_string(),
                arguments: "invalid json".to_string(), // This should cause an error
            },
        },
        LlmMessage::Done,
    ]);

    let mut agent = Agent::new(fake_llm, Some("You are a helpful assistant.".to_string()));

    let stream = agent.send_message("Calculate something").await;
    pin_mut!(stream);

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have a tool call chunk with an error result
    let has_error_result = events.iter().any(|e| match e {
        AgentMessage::ToolCallChunk {
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
