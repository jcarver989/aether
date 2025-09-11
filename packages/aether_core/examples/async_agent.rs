use aether_core::{
    agent::{Agent, AgentEvent},
    llm::ollama::LocalLlmProvider,
    tools::ToolRegistry,
};
use futures::pin_mut;
use tokio_stream::StreamExt;

#[tokio::main]
pub async fn main() {
    println!("Hello world");

    let (client_tx, mut client_rx) = tokio::sync::mpsc::channel::<AgentEvent>(100);
    let (agent_tx, mut agent_rx) = tokio::sync::mpsc::channel::<&str>(100);

    let _ = tokio::spawn(async move {
        let provider = LocalLlmProvider::new_llama_cpp().unwrap();
        let tools = ToolRegistry::new();
        let mut agent = Agent::new(provider, tools, Some("you are a helpful agent".to_string()));

        while let Some(message) = agent_rx.recv().await {
            let result_stream = agent.send_message(message).await;
            pin_mut!(result_stream);

            while let Some(event) = result_stream.next().await {
                match client_tx.send(event).await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error sending agent event: {}", e);
                        break;
                    }
                }
            }
        }
    });

    agent_tx.send("What is 5+5?").await.unwrap();

    while let Some(event) = client_rx.recv().await {
        match event {
            AgentEvent::MessageChunk {
                chunk, is_complete, ..
            } => {
                if is_complete {
                    println!(); // New line when message is complete
                } else {
                    print!("{}", chunk);
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                }
            }

            AgentEvent::ToolCallChunk {
                name, is_complete, ..
            } => {
                if is_complete {
                    println!("Tool call '{}' completed", name);
                } else {
                    println!("Tool call '{}' started", name);
                }
            }

            AgentEvent::Error { message } => {
                eprintln!("Error: {}", message);
            }
        }
    }
}
