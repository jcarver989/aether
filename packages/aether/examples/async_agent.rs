use aether::{
    agent::{AgentMessage, UserMessage, agent},
    llm::local::DefaultModelProvider,
};

#[tokio::main]
pub async fn main() -> color_eyre::Result<()> {
    println!("Hello world");

    let provider = DefaultModelProvider::llama_cpp()?;
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
        }
    }

    Ok(())
}
