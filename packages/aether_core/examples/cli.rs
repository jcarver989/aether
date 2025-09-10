use aether_core::{
    agent::Agent,
    llm::ollama::OllamaProvider,
    tools::ToolRegistry,
    types::{ChatMessage, IsoString, StreamEvent},
};
use tokio_stream::StreamExt;

#[tokio::main]
pub async fn main() {
    println!("Hello world");

    let provider = OllamaProvider::new(None, "gemma3").unwrap();
    let tools = ToolRegistry::new();
    let mut agent = Agent::new(provider, tools, Some("you are a helpful agent".to_string()));

    agent.add_message(ChatMessage::User {
        content: "Say hello".to_string(),
        timestamp: IsoString::now(),
    });

    let mut stream = agent.stream_completion(None).await.unwrap();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.unwrap();
        match chunk {
            StreamEvent::Content { chunk: content } => {
                agent.append_streaming_content(&content);
            }
            StreamEvent::Done => {
                agent.finalize_streaming_message();
                break;
            }
            _ => {}
        }
    }
}
