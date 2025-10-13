use aether::{
    agent::{AgentMessage, UserMessage, agent},
    llm::openrouter::OpenRouterProvider,
    mcp::mcp_builder::mcp,
};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let llm = OpenRouterProvider::default("z-ai/glm-4.5-air")?;
    let (tools, mcp_tx, _mcp_handle) = mcp().mcp_json_file("examples/mcp.json")?.spawn().await?;
    let (tx, mut rx, _handle) = agent(llm)
        .system("You are a helpful assistant with access to web browsing tools via Playwright.")
        .mcp_tools(mcp_tx, tools)
        .spawn()
        .await?;

    tx.send(UserMessage::text(
        "Visit https://contextbridge.ai and tell me what you see",
    ))
    .await?;

    loop {
        use AgentMessage::*;
        match rx.recv().await {
            Some(Text {
                chunk, is_complete, ..
            }) => {
                if !is_complete {
                    print!("{}", chunk);
                    io::stdout().flush().unwrap();
                } else {
                    println!();
                }
            }
            Some(ToolCall { name, result, .. }) => {
                if result.is_some() {
                    println!("\n🔧 Tool '{}' executed", name);
                }
            }
            Some(Done) => {
                println!("\n✓ Agent finished");
                break;
            }
            Some(Error { message }) => {
                eprintln!("❌ Error: {}", message);
                break;
            }
            Some(Cancelled { .. }) => {
                println!("⚠️  Cancelled");
                break;
            }
            Some(ElicitationRequest { .. }) => {}
            None => {
                println!("Channel closed");
                break;
            }
        }
    }

    Ok(())
}
