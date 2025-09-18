use aether::{
    agent::{AgentMessage, UserMessage, agent},
    testing::FakeLlmProvider,
    types::LlmResponse,
};
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn test_agent_spawn_basic_communication() {
    // Create a fake LLM with a simple response
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::Text {
            chunk: "Hello".to_string(),
        },
        LlmResponse::Text {
            chunk: " World".to_string(),
        },
        LlmResponse::Done,
    ]);

    // Spawn the agent
    let mut spawned_agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant")
        .spawn()
        .await
        .expect("Failed to spawn agent");

    // Send a message to the agent
    let user_message = UserMessage::text("Test message");
    spawned_agent
        .tx
        .send(user_message)
        .await
        .expect("Failed to send message");

    // Collect responses with timeout
    let mut responses = Vec::new();
    let timeout_duration = Duration::from_secs(5);

    // Collect all responses until the stream ends or timeout
    loop {
        match timeout(timeout_duration, spawned_agent.rx.recv()).await {
            Ok(Some(message)) => {
                match &message {
                    AgentMessage::Text {
                        is_complete: true, ..
                    } => {
                        responses.push(message);
                        break; // Stream is complete
                    }
                    _ => responses.push(message),
                }
            }
            Ok(None) => break, // Channel closed
            Err(_) => {
                panic!("Timeout waiting for agent response");
            }
        }
    }

    // Verify we got some responses
    assert!(
        !responses.is_empty(),
        "Should have received at least one response"
    );

    // Verify we got text messages
    let text_messages: Vec<_> = responses
        .iter()
        .filter_map(|msg| match msg {
            AgentMessage::Text { chunk, .. } => Some(chunk),
            _ => None,
        })
        .collect();

    assert!(
        !text_messages.is_empty(),
        "Should have received text messages"
    );

    // Clean shutdown
    spawned_agent.task_handle.abort();
}

#[tokio::test]
async fn test_agent_spawn_multiple_messages() {
    // Create a fake LLM with multiple responses
    let fake_llm = FakeLlmProvider::new(vec![
        vec![
            LlmResponse::Start {
                message_id: "msg1".to_string(),
            },
            LlmResponse::Text {
                chunk: "First response".to_string(),
            },
            LlmResponse::Done,
        ],
        vec![
            LlmResponse::Start {
                message_id: "msg2".to_string(),
            },
            LlmResponse::Text {
                chunk: "Second response".to_string(),
            },
            LlmResponse::Done,
        ],
    ]);

    // Spawn the agent
    let mut spawned_agent = agent(fake_llm)
        .spawn()
        .await
        .expect("Failed to spawn agent");

    let timeout_duration = Duration::from_secs(5);

    // Send first message
    spawned_agent
        .tx
        .send(UserMessage::text("First message"))
        .await
        .expect("Failed to send first message");

    // Collect first response
    let mut first_responses = Vec::new();
    loop {
        match timeout(timeout_duration, spawned_agent.rx.recv()).await {
            Ok(Some(message)) => match &message {
                AgentMessage::Text {
                    is_complete: true, ..
                } => {
                    first_responses.push(message);
                    break;
                }
                _ => first_responses.push(message),
            },
            Ok(None) => break,
            Err(_) => panic!("Timeout waiting for first response"),
        }
    }

    // Send second message
    spawned_agent
        .tx
        .send(UserMessage::text("Second message"))
        .await
        .expect("Failed to send second message");

    // Collect second response
    let mut second_responses = Vec::new();
    loop {
        match timeout(timeout_duration, spawned_agent.rx.recv()).await {
            Ok(Some(message)) => match &message {
                AgentMessage::Text {
                    is_complete: true, ..
                } => {
                    second_responses.push(message);
                    break;
                }
                _ => second_responses.push(message),
            },
            Ok(None) => break,
            Err(_) => panic!("Timeout waiting for second response"),
        }
    }

    // Verify both responses were received
    assert!(
        !first_responses.is_empty(),
        "Should have received first response"
    );
    assert!(
        !second_responses.is_empty(),
        "Should have received second response"
    );

    // Clean shutdown
    spawned_agent.task_handle.abort();
}

#[tokio::test]
async fn test_agent_spawn_task_cleanup() {
    // Create a fake LLM with a simple response
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::Text {
            chunk: "Test".to_string(),
        },
        LlmResponse::Done,
    ]);

    // Spawn the agent
    let spawned_agent = agent(fake_llm)
        .spawn()
        .await
        .expect("Failed to spawn agent");

    // Drop the channels to signal shutdown
    drop(spawned_agent.tx);
    drop(spawned_agent.rx);

    // Wait for the task to complete gracefully
    let result = timeout(Duration::from_secs(5), spawned_agent.task_handle).await;
    assert!(
        result.is_ok(),
        "Task should complete gracefully when channels are dropped"
    );
}
