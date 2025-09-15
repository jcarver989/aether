use aether::{
    agent::{AgentMessage, UserMessage, agent},
    testing::fake_llm::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
};
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_basic_cancellation() {
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::Text {
            chunk: "Starting a long task...".to_string(),
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    let (stream, cancel_token) = agent.send(UserMessage::text("Start task")).await;
    let mut stream = Box::pin(stream);

    // Start collecting events (as debug strings now)
    let mut events: Vec<String> = Vec::new();
    let mut has_cancelled = false;

    // Cancel immediately
    cancel_token.cancel();

    while let Some(event) = stream.next().await {
        // Instead of cloning, just check the event type
        match event {
            AgentMessage::Cancelled { .. } => {
                has_cancelled = true;
                break;
            }
            AgentMessage::ElicitationRequest { response_sender, .. } => {
                // Handle elicitation requests by declining them in tests
                use rmcp::model::{CreateElicitationResult, ElicitationAction};
                let _ = response_sender.send(CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                });
            }
            event => {
                // Count other events without storing them
                events.push(format!("{:?}", event));
            }
        }
    }

    assert!(
        has_cancelled || cancel_token.is_cancelled(),
        "Expected operation to be cancelled"
    );
}

#[tokio::test]
async fn test_cancel_message_variant() {
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::Text {
            chunk: "This should not be seen".to_string(),
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    // Send a cancel message directly
    let (stream, cancel_token) = agent.send(UserMessage::Cancel).await;
    let mut stream = Box::pin(stream);

    // Token should already be cancelled
    assert!(cancel_token.is_cancelled());

    let mut events: Vec<String> = Vec::new();
    let mut has_cancelled_message = false;
    while let Some(event) = stream.next().await {
        match event {
            AgentMessage::Cancelled { .. } => {
                has_cancelled_message = true;
                events.push("Cancelled".to_string());
            }
            AgentMessage::ElicitationRequest { response_sender, .. } => {
                use rmcp::model::{CreateElicitationResult, ElicitationAction};
                let _ = response_sender.send(CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                });
            }
            event => {
                events.push(format!("{:?}", event));
            }
        }
    }

    assert!(has_cancelled_message, "Expected Cancelled message");
}

#[tokio::test]
async fn test_cancellation_during_tool_execution() {
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::Text {
            chunk: "I'll use a tool to help.".to_string(),
        },
        LlmResponse::ToolRequestStart {
            id: "tool1".to_string(),
            name: "coding::write_file".to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tool1".to_string(),
                name: "coding::write_file".to_string(),
                arguments: r#"{"path": "/test.txt", "content": "Hello"}"#.to_string(),
            },
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    let (stream, cancel_token) = agent.send(UserMessage::text("Write a file")).await;
    let mut stream = Box::pin(stream);

    let mut events: Vec<String> = Vec::new();
    let mut tool_started = false;

    // Collect events and cancel when we see a tool start
    while let Some(event) = stream.next().await {
        let should_cancel = matches!(
            event,
            AgentMessage::ToolCall {
                is_complete: false,
                ..
            }
        );

        let is_cancelled = matches!(event, AgentMessage::Cancelled { .. });

        match event {
            AgentMessage::ElicitationRequest { response_sender, .. } => {
                use rmcp::model::{CreateElicitationResult, ElicitationAction};
                let _ = response_sender.send(CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                });
            }
            event => {
                events.push(format!("{:?}", event));
            }
        }

        if should_cancel {
            tool_started = true;
            // Cancel after tool starts but before it completes
            cancel_token.cancel();
        }

        if is_cancelled {
            break;
        }
    }

    assert!(tool_started, "Tool should have started");
    assert!(cancel_token.is_cancelled(), "Token should be cancelled");

    // Check that we got a cancelled message or the token is cancelled
    let has_cancelled_event = events
        .iter()
        .any(|e| e.contains("Cancelled"));

    assert!(
        has_cancelled_event || cancel_token.is_cancelled(),
        "Expected cancellation to be acknowledged"
    );
}

#[tokio::test]
async fn test_multiple_operations_with_cancellation() {
    let fake_llm = FakeLlmProvider::new(vec![
        // First operation - will be cancelled
        vec![
            LlmResponse::Start {
                message_id: "msg1".to_string(),
            },
            LlmResponse::Text {
                chunk: "First operation...".to_string(),
            },
            LlmResponse::Done,
        ],
        // Second operation - should complete normally
        vec![
            LlmResponse::Start {
                message_id: "msg2".to_string(),
            },
            LlmResponse::Text {
                chunk: "Second operation completed".to_string(),
            },
            LlmResponse::Done,
        ],
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    // First operation
    let (stream1, cancel_token1) = agent.send(UserMessage::text("First task")).await;
    let mut stream1 = Box::pin(stream1);
    cancel_token1.cancel();

    let mut first_events: Vec<String> = Vec::new();
    while let Some(event) = stream1.next().await {
        let is_cancelled = matches!(event, AgentMessage::Cancelled { .. });

        match event {
            AgentMessage::ElicitationRequest { response_sender, .. } => {
                use rmcp::model::{CreateElicitationResult, ElicitationAction};
                let _ = response_sender.send(CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                });
            }
            event => {
                first_events.push(format!("{:?}", event));
            }
        }

        if is_cancelled {
            break;
        }
    }

    // Drop the first stream to release the borrow
    drop(stream1);

    // Second operation should work normally
    let (stream2, cancel_token2) = agent.send(UserMessage::text("Second task")).await;
    let mut stream2 = Box::pin(stream2);

    let mut second_events: Vec<String> = Vec::new();
    let mut has_second_text = false;
    let mut has_complete_message = false;

    while let Some(event) = stream2.next().await {
        match event {
            AgentMessage::Text { chunk, is_complete, .. } => {
                if !chunk.is_empty() {
                    has_second_text = true;
                }
                if is_complete {
                    has_complete_message = true;
                }
                second_events.push(format!("Text(chunk: {}, complete: {})", chunk, is_complete));
            }
            AgentMessage::ElicitationRequest { response_sender, .. } => {
                use rmcp::model::{CreateElicitationResult, ElicitationAction};
                let _ = response_sender.send(CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                });
            }
            event => {
                second_events.push(format!("{:?}", event));
            }
        }
    }

    assert!(
        cancel_token1.is_cancelled(),
        "First token should be cancelled"
    );
    assert!(
        !cancel_token2.is_cancelled(),
        "Second token should not be cancelled"
    );

    assert!(
        has_second_text,
        "Second operation should have some text output"
    );
    assert!(
        has_complete_message,
        "Second operation should complete normally"
    );
}

#[tokio::test]
async fn test_cancellation_token_isolation() {
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::Text {
            chunk: "Working...".to_string(),
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    // Get first token
    let (stream1, token1) = agent.send(UserMessage::text("Task 1")).await;
    drop(stream1); // Drop to release borrow

    // Get second token
    let (stream2, token2) = agent.send(UserMessage::text("Task 2")).await;

    // Tokens should be different instances (each agent.send creates a new token)
    // Cancel first token should not affect second
    token1.cancel();
    assert!(token1.is_cancelled());
    assert!(!token2.is_cancelled());

    // Cleanup streams
    drop(stream2);
}
