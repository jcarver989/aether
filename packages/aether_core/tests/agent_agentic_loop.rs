use aether_core::{
    agent::{agent, AgentMessage, UserMessage},
    testing::fake_llm::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
};
use tokio_stream::StreamExt;

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
        .system("You are a helpful assistant.")
        .coding_tools()
        .build()
        .await
        .unwrap();

    let mut stream = Box::pin(agent.send(UserMessage::text("Write a test file")).await);


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
async fn test_recursive_tool_calls() {
    // Create a fake LLM that makes multiple rounds of tool calls
    let fake_llm = FakeLlmProvider::new(vec![
        // First LLM response with a tool call
        vec![
            LlmResponse::Start {
                message_id: "msg1".to_string(),
            },
            LlmResponse::Text {
                chunk: "I need to gather some information first.".to_string(),
            },
            LlmResponse::ToolRequestStart {
                id: "tool1".to_string(),
                name: "coding::read_file".to_string(),
            },
            LlmResponse::ToolRequestComplete {
                tool_call: ToolCallRequest {
                    id: "tool1".to_string(),
                    name: "coding::read_file".to_string(),
                    arguments: r#"{"path": "/data.txt"}"#.to_string(),
                },
            },
            LlmResponse::Done,
        ],
        // Second LLM response with another tool call based on the first result
        vec![
            LlmResponse::Start {
                message_id: "msg2".to_string(),
            },
            LlmResponse::Text {
                chunk: "Now I'll process that information.".to_string(),
            },
            LlmResponse::ToolRequestStart {
                id: "tool2".to_string(),
                name: "coding::write_file".to_string(),
            },
            LlmResponse::ToolRequestComplete {
                tool_call: ToolCallRequest {
                    id: "tool2".to_string(),
                    name: "coding::write_file".to_string(),
                    arguments: r#"{"path": "/output.txt", "content": "processed data"}"#
                        .to_string(),
                },
            },
            LlmResponse::Done,
        ],
        // Final LLM response with no tool calls (completion)
        vec![
            LlmResponse::Start {
                message_id: "msg3".to_string(),
            },
            LlmResponse::Text {
                chunk: "Here's your final result based on the processed data.".to_string(),
            },
            LlmResponse::Done,
        ],
    ]);

    let mut agent = agent(fake_llm)
        .system("You are a helpful assistant.")
        .coding_tools()
        .build()
        .await
        .unwrap();

    let mut stream = Box::pin(agent.send(UserMessage::text("Process my data")).await);

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
                AgentMessage::Text {
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
                AgentMessage::ToolCall {
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
        LlmResponse::Start {
            message_id: "msg".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "tool".to_string(),
            name: "coding::list_files".to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool".to_string(),
                name: "coding::list_files".to_string(),
                arguments: r#"{"path": "/"}"#.to_string(),
            },
        },
        LlmResponse::Done,
    ];

    // Create responses that repeat the tool call pattern
    let mut responses = Vec::new();
    for _ in 0..15 {
        // More than MAX_RECURSION_DEPTH (10)
        responses.push(tool_call_response.clone());
    }

    let fake_llm = FakeLlmProvider::new(responses);
    let mut agent = agent(fake_llm)
        .system("You are a helpful assistant.")
        .coding_tools()
        .build()
        .await
        .unwrap();

    let mut stream = Box::pin(agent.send(UserMessage::text("Start endless loop")).await);

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
        .system("You are a helpful assistant.")
        .coding_tools()
        .build()
        .await
        .unwrap();

    let mut stream = Box::pin(agent.send(UserMessage::text("Write a file")).await);

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
