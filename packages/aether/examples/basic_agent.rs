use aether::{
    agent::{AgentMessage, SystemPrompt, UserMessage, agent},
    llm::openrouter::OpenRouterProvider,
};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let llm = OpenRouterProvider::default("z-ai/glm-4.5-air")?;
    let mut agent = agent(llm)
        .system(&[SystemPrompt::text(
            "You are a helpful assistant. Keep your responses concise.",
        )])
        .spawn()
        .await?;

    agent
        .send(UserMessage::text("Write one paragraph about a unicorn"))
        .await?;

    loop {
        use AgentMessage::*;
        match agent.recv().await {
            Some(Text {
                chunk, is_complete, ..
            }) => {
                if !is_complete {
                    print!("{}", chunk);
                    io::stdout().flush().unwrap();
                } else {
                    println!("\n\n✓ Message complete");
                }
            }
            Some(ToolCall { name, result, .. }) => {
                if let Some(res) = result {
                    println!("🔧 Tool '{}' completed", name);
                    println!("   Result: {}", res);
                }
            }
            Some(Done) => {
                println!("✓ Agent finished processing");
                break;
            }
            Some(Error { message }) => {
                eprintln!("❌ Error: {}", message);
                break;
            }
            Some(Cancelled { .. }) => {
                println!("⚠️  Processing cancelled");
                break;
            }
            Some(ElicitationRequest { .. }) => {
                // Ignore elicitation requests in this simple example
            }
            None => {
                println!("Channel closed");
                break;
            }
        }
    }

    Ok(())
}
