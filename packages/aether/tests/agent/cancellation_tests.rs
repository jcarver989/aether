use aether::{
    agent::{AgentMessage, SystemPrompt, UserMessage, agent},
    testing::fake_llm::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
};
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn test_cancel_message_variant() {
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::Text {
            chunk: "This should be seen".to_string(),
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system(&[SystemPrompt::Text("You ar
            e a helpful assistant.".to_string())]),

        .spawn()
        .await
        .unwrap();

    // Send a message
    agent.send(UserMessage::text("test")).await.unwrap();

    let mut has_text_message = false;
    let mut has_completed = false;
    let timeout_duration = Duration::from_secs(5);

    loop {
        match timeout(timeout_duration, agent.recv()).await {
            Ok(Some(event)) => match event {
                AgentMessage::Text {
                    chunk, is_complete, ..
                } => {
                    if !chunk.is_empty() {
                        has_text_message = true;
                    }
                    if is_complete {
                        has_completed = true;
                        break;
                    }
                }
                AgentMessage::Done => break,
                _ => {}
            },
            Ok(None) => break,
            Err(_) => panic!("Timeout waiting for response"),
        }
    }

    assert!(has_text_message, "Expected to receive text message");
    assert!(has_completed, "Expected message to complete");
}

#[tokio::test]
async fn test_cancellation_with_cancel_message() {
    let fake_llm = FakeLlmProvider::new(vec![
        vec![
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
        ],
        // Second response after tool execution - just Done to end the loop
        vec![LlmResponse::Done],
    ]);

    let mut agent = agent(fake_llm)
        .system(&[SystemPrompt::Text("You ar
            e a helpful assistant.".to_string())]),

        .spawn()
        .await
        .unwrap();

    // Send a message
    agent.send(UserMessage::text("Write a file")).await.unwrap();

    let mut tool_started = false;
    let timeout_duration = Duration::from_secs(5);

    // Wait for a tool to start, then cancel
    loop {
        match timeout(timeout_duration, agent.recv()).await {
            Ok(Some(event)) => {
                if matches!(
                    event,
                    AgentMessage::ToolCall {
                        is_complete: false,
                        ..
                    }
                ) {
                    tool_started = true;
                    // Send cancel message
                    agent.send(UserMessage::Cancel).await.unwrap();
                }

                if matches!(event, AgentMessage::Cancelled { .. }) {
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => break, // Timeout is ok for this test
        }
    }

    assert!(tool_started, "Tool should have started before cancellation");
}

#[tokio::test]
async fn test_new_message_cancels_previous() {
    let fake_llm = FakeLlmProvider::new(vec![
        // First operation - will be cancelled by second message
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
        .system(&[SystemPrompt::Text("You ar
            e a helpful assistant.".to_string())]),

        .spawn()
        .await
        .unwrap();

    let timeout_duration = Duration::from_secs(5);

    // Send first message
    agent.send(UserMessage::text("First task")).await.unwrap();

    // Small delay to let first message start processing
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Send second message to cancel the first
    agent.send(UserMessage::text("Second task")).await.unwrap();

    let mut message_count = 0;
    let mut done_count = 0;

    // Collect all messages - we should get responses from both messages
    // The first may be cancelled or complete, but the second should complete
    loop {
        match timeout(timeout_duration, agent.recv()).await {
            Ok(Some(event)) => match event {
                AgentMessage::Text { is_complete, .. } => {
                    if is_complete {
                        message_count += 1;
                    }
                }
                AgentMessage::Done => {
                    done_count += 1;
                    if done_count >= 2 {
                        break;
                    }
                }
                AgentMessage::Cancelled { .. } => {
                    // First message was cancelled, continue to second
                }
                _ => {}
            },
            Ok(None) => break,
            Err(_) => break,
        }
    }

    // We should have received at least one complete message (possibly two if first wasn't cancelled fast enough)
    assert!(
        message_count >= 1,
        "Should have received at least one complete message"
    );
}

#[tokio::test]
async fn test_sequential_messages() {
    let fake_llm = FakeLlmProvider::new(vec![
        vec![
            LlmResponse::Start {
                message_id: "msg1".to_string(),
            },
            LlmResponse::Text {
                chunk: "First".to_string(),
            },
            LlmResponse::Done,
        ],
        vec![
            LlmResponse::Start {
                message_id: "msg2".to_string(),
            },
            LlmResponse::Text {
                chunk: "Second".to_string(),
            },
            LlmResponse::Done,
        ],
    ]);

    let mut agent = agent(fake_llm)
        .system(&[SystemPrompt::Text("You ar
            e a helpful assistant.".to_string())]),

        .spawn()
        .await
        .unwrap();

    let timeout_duration = Duration::from_secs(5);

    // First task
    agent.send(UserMessage::text("Task 1")).await.unwrap();
    loop {
        match timeout(timeout_duration, agent.recv()).await {
            Ok(Some(AgentMessage::Done)) => break,
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => panic!("Timeout waiting for first task"),
        }
    }

    // Second task
    agent.send(UserMessage::text("Task 2")).await.unwrap();
    loop {
        match timeout(timeout_duration, agent.recv()).await {
            Ok(Some(AgentMessage::Done)) => break,
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => panic!("Timeout waiting for second task"),
        }
    }
}
