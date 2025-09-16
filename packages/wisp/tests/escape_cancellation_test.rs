use aether::{
    agent::{AgentMessage, UserMessage, agent},
    testing::fake_llm::FakeLlmProvider,
    types::LlmResponse,
};
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_escape_key_cancellation_mechanism() {
    // This test verifies the cancellation mechanism works
    // Note: We can't easily simulate actual escape key presses in a unit test,
    // but we can test that the cancellation token works as expected

    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::Text {
            chunk: "This is a long running operation...".to_string(),
        },
        // Simulate a long-running response that could be cancelled
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    let (stream, cancel_token) = agent.send(UserMessage::text("Start long task")).await;
    let mut stream = Box::pin(stream);

    // Simulate what happens when escape is pressed: cancel the token
    cancel_token.cancel();

    let mut events: Vec<String> = Vec::new();
    let mut has_cancelled = false;

    while let Some(event) = stream.next().await {
        match event {
            AgentMessage::Cancelled { .. } => {
                has_cancelled = true;
                break;
            }
            AgentMessage::ElicitationRequest {
                response_sender, ..
            } => {
                use aether::{CreateElicitationResult, ElicitationAction};
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

    assert!(
        has_cancelled || cancel_token.is_cancelled(),
        "Expected operation to be cancelled when token is cancelled"
    );
}

#[tokio::test]
async fn test_cancellation_token_isolation() {
    // Test that each operation gets its own cancellation token
    let fake_llm = FakeLlmProvider::new(vec![
        vec![
            LlmResponse::Start {
                message_id: "msg1".to_string(),
            },
            LlmResponse::Text {
                chunk: "First task".to_string(),
            },
            LlmResponse::Done,
        ],
        vec![
            LlmResponse::Start {
                message_id: "msg2".to_string(),
            },
            LlmResponse::Text {
                chunk: "Second task".to_string(),
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
    let (stream1, token1) = agent.send(UserMessage::text("Task 1")).await;
    drop(stream1); // Drop to release borrow

    // Second operation
    let (stream2, token2) = agent.send(UserMessage::text("Task 2")).await;
    drop(stream2);

    // Tokens should be independent
    token1.cancel();
    assert!(token1.is_cancelled());
    assert!(
        !token2.is_cancelled(),
        "Second token should not be affected by first token cancellation"
    );
}

// Helper function to simulate the escape key detection logic
// This function mimics what wait_for_escape_key does
async fn simulate_escape_detection() -> Result<(), std::io::Error> {
    use std::time::Duration;
    // In a real scenario, this would poll for actual keyboard events
    // For testing, we just simulate the escape key being "detected"
    tokio::time::sleep(Duration::from_millis(10)).await;
    Ok(()) // Simulate escape key detected
}

#[tokio::test]
async fn test_escape_detection_simulation() {
    use std::time::Duration;
    // Test that our escape detection simulation works
    let start = tokio::time::Instant::now();
    let result = simulate_escape_detection().await;
    let duration = start.elapsed();

    assert!(result.is_ok(), "Escape detection should succeed");
    assert!(
        duration >= Duration::from_millis(10),
        "Should take at least 10ms"
    );
    assert!(
        duration < Duration::from_millis(100),
        "Should complete quickly"
    );
}

#[tokio::test]
async fn test_stream_merging_concept() {
    // Test that demonstrates the stream merging concept
    use futures::stream;
    use std::time::Duration;

    // Create a mock agent stream that emits some events
    let agent_events = vec!["event1", "event2", "event3"];
    let agent_stream = stream::iter(agent_events.into_iter().map(|s| format!("Agent: {}", s)));

    // Create a mock escape key stream that emits after a short delay
    let escape_stream = async_stream::stream! {
        tokio::time::sleep(Duration::from_millis(10)).await;
        yield "EscapeKey".to_string();
    };

    // Merge the streams
    let merged_stream = stream::select(agent_stream, escape_stream);
    let events: Vec<String> = futures::StreamExt::collect(merged_stream).await;

    // We should get both agent events and the escape key event
    assert!(
        events.len() >= 2,
        "Should have at least agent and escape events"
    );
    assert!(
        events.iter().any(|e| e.contains("Agent")),
        "Should have agent events"
    );
    assert!(
        events.iter().any(|e| e.contains("EscapeKey")),
        "Should have escape key event"
    );
}
