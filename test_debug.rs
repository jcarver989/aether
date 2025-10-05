use aether::{
    agent::{AgentMessage, UserMessage, agent},
    testing::FakeLlmProvider,
    types::LlmResponse,
};

#[tokio::main]
async fn main() {
    let llm = FakeLlmProvider::with_single_response(vec![
        LlmResponse::Start {
            message_id: "test".to_string(),
        },
        LlmResponse::Text {
            chunk: "Hello".to_string(),
        },
        LlmResponse::Done,
    ]);

    let mut agent = agent(llm)
        .spawn()
        .await
        .unwrap();

    println!("Agent spawned");

    // Send a message
    println!("Sending message...");
    agent.send(UserMessage::text("test")).await.unwrap();
    println!("Message sent");

    // Receive response
    println!("Waiting for response...");
    match tokio::time::timeout(std::time::Duration::from_secs(2), agent.recv()).await {
        Ok(Some(msg)) => println!("Got message: {:?}", msg),
        Ok(None) => println!("Channel closed"),
        Err(_) => println!("Timeout!"),
    }
}
