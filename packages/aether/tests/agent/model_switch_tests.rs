use aether::core::{COMPLETION_SIGNAL, agent};
use aether::events::{AgentMessage, UserMessage};
use llm::LlmResponse;
use llm::testing::FakeLlmProvider;

#[tokio::test]
async fn test_switch_model_emits_model_switched() {
    // The switched-to provider will produce this response
    let switch_responses = vec![vec![
        LlmResponse::start("after-switch"),
        LlmResponse::text("Switched! "),
        LlmResponse::text(COMPLETION_SIGNAL),
        LlmResponse::Done,
    ]];
    let new_provider = FakeLlmProvider::new(switch_responses);

    // Initial LLM produces a response, then we switch
    let initial_responses = vec![vec![
        LlmResponse::start("msg-1"),
        LlmResponse::text("Hello "),
        LlmResponse::text(COMPLETION_SIGNAL),
        LlmResponse::Done,
    ]];
    let llm = FakeLlmProvider::new(initial_responses);

    let (tx, mut rx, _handle) = agent(llm).spawn().await.unwrap();

    // Send initial message
    tx.send(UserMessage::text("hi")).await.unwrap();

    // Wait for initial response to complete
    let mut got_initial_done = false;
    while let Some(msg) = rx.recv().await {
        if matches!(msg, AgentMessage::Done) {
            got_initial_done = true;
            break;
        }
    }
    assert!(got_initial_done, "Expected Done after initial message");

    // Switch models by sending a ready-to-use provider
    tx.send(UserMessage::SwitchModel(Box::new(new_provider)))
        .await
        .unwrap();

    // Send a follow-up message to exercise the new provider
    tx.send(UserMessage::text("after switch")).await.unwrap();
    drop(tx);

    // Collect remaining messages
    let mut messages = Vec::new();
    while let Some(msg) = rx.recv().await {
        messages.push(msg);
    }

    // Should have ModelSwitched with display name strings
    let switched = messages
        .iter()
        .find(|m| matches!(m, AgentMessage::ModelSwitched { .. }));
    assert!(
        switched.is_some(),
        "Expected ModelSwitched message, got: {messages:?}"
    );
    if let Some(AgentMessage::ModelSwitched { previous, new }) = switched {
        // FakeLlmProvider::display_name() returns "Fake LLM"
        assert_eq!(previous, "Fake LLM");
        assert_eq!(new, "Fake LLM");
    }
}
