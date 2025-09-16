use aether::{
    agent::{AgentMessage, UserMessage, agent},
    llm::local::llama_cpp::LlamaCppProvider,
};

#[tokio::main]
pub async fn main() -> color_eyre::Result<()> {
    println!("Hello world");

    let provider = LlamaCppProvider::default();
    let (tx, mut rx) = agent(provider)
        .system_prompt("you are a helpful agent")
        .spawn()
        .await?;

    tx.send(UserMessage::text("What is 5+5?")).await.unwrap();

    while let Some(event) = rx.recv().await {
        match event {
            AgentMessage::Text {
                chunk, is_complete, ..
            } => {
                if is_complete {
                    println!(); // New line when message is complete
                } else {
                    print!("{}", chunk);
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                }
            }

            AgentMessage::ToolCall {
                name, is_complete, ..
            } => {
                if is_complete {
                    println!("Tool call '{}' completed", name);
                } else {
                    println!("Tool call '{}' started", name);
                }
            }

            AgentMessage::Error { message } => {
                eprintln!("Error: {}", message);
            }

            AgentMessage::Cancelled { message } => {
                eprintln!("Cancelled: {}", message);
            }

            AgentMessage::ElicitationRequest {
                request_id,
                request,
                response_sender,
            } => {
                println!("Elicitation request ({}): {}", request_id, request.message);
                // For this example, just decline all elicitation requests
                use rmcp::model::{CreateElicitationResult, ElicitationAction};
                let result = CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                };
                let _ = response_sender.send(result);
            }
        }
    }

    Ok(())
}
