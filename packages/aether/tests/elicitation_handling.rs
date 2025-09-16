use aether::{
    agent::{AgentMessage, UserMessage, agent},
    testing::fake_llm::FakeLlmProvider,
    types::{LlmResponse, ToolCallRequest},
};
use rmcp::model::{CreateElicitationResult, ElicitationAction};
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_elicitation_request_handling() {
    // Create a fake LLM that will trigger an elicitation request
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tc1".to_string(),
                name: "test_tool".to_string(),
                arguments: r#"{"action": "sensitive_operation"}"#.to_string(),
            },
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    let (stream, _cancel_token) = agent.send(UserMessage::text("Perform action")).await;
    let mut stream = Box::pin(stream);

    let mut elicitation_received = false;

    while let Some(event) = stream.next().await {
        match event {
            AgentMessage::ElicitationRequest {
                request_id,
                request,
                response_sender,
            } => {
                elicitation_received = true;

                // Verify the request structure
                assert!(!request_id.is_empty());
                assert!(!request.message.is_empty());

                // Simulate approving the request (like our CLI implementation would)
                let result = CreateElicitationResult {
                    action: ElicitationAction::Accept,
                    content: None,
                };

                let _ = response_sender.send(result);
            }
            AgentMessage::Error { message } => {
                panic!("Unexpected error: {}", message);
            }
            _ => {
                // Other message types are fine
            }
        }
    }

    assert!(
        elicitation_received,
        "Expected an elicitation request to be received"
    );
}

#[tokio::test]
async fn test_elicitation_request_decline() {
    let fake_llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "msg1".to_string(),
        },
        LlmResponse::ToolRequestComplete {
            tool_call: ToolCallRequest {
                id: "tc1".to_string(),
                name: "test_tool".to_string(),
                arguments: r#"{"action": "sensitive_operation"}"#.to_string(),
            },
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(fake_llm)
        .system_prompt("You are a helpful assistant.")
        .build()
        .await
        .unwrap();

    let (stream, _cancel_token) = agent.send(UserMessage::text("Perform action")).await;
    let mut stream = Box::pin(stream);

    let mut elicitation_received = false;

    while let Some(event) = stream.next().await {
        match event {
            AgentMessage::ElicitationRequest {
                response_sender, ..
            } => {
                elicitation_received = true;

                // Simulate declining the request
                let result = CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                };

                let _ = response_sender.send(result);
            }
            AgentMessage::Error { message } => {
                // This might be expected if the tool call was declined
                println!("Tool call declined, received error: {}", message);
            }
            _ => {
                // Other message types are fine
            }
        }
    }

    assert!(
        elicitation_received,
        "Expected an elicitation request to be received"
    );
}
