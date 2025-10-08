use aether::{
    agent::{AgentEvent, MiddlewareAction, Prompt, agent},
    llm::openrouter::OpenRouterProvider,
};
use tokio::process::Command;

/// Example demonstrating middleware/hooks functionality
///
/// This shows how to add event handlers that can observe and control agent behavior.
/// Handlers can block actions (like dangerous tool calls) before they execute.
/// Great for security controls, logging, and cross-cutting concerns.

/// Helper to run shell commands asynchronously
async fn run_command(cmd: &str) {
    let _ = Command::new("sh").arg("-c").arg(cmd).output().await;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up the LLM provider
    let llm = OpenRouterProvider::default("anthropic/claude-3.5-sonnet")?;

    // Create agent with middleware hooks
    let prompt = Prompt::text("You are a helpful assistant").build()?;

    let _agent = agent(llm)
        .system(&prompt)
        // Middleware hook that handles events before they execute
        .on_event(|event| async move {
            match event {
                AgentEvent::UserMessage { content } => {
                    run_command(&format!("echo '[USER] {}' >> conversation.log", content)).await;
                }
                AgentEvent::ToolCall {
                    name, arguments, ..
                } => {
                    println!("🔧 Tool called: {}", name);
                    println!("   Arguments: {}", arguments);
                    run_command(&format!(
                        "echo '[TOOL] {} with args {}' >> conversation.log",
                        name, arguments
                    ))
                    .await;

                    // Example: Block dangerous tools
                    if name == "rm" || name == "delete" {
                        println!("❌ Blocked dangerous tool: {}", name);
                        return MiddlewareAction::Block;
                    }
                }
            }
            MiddlewareAction::Allow
        })
        .spawn()
        .await?;

    println!("Agent spawned with middleware hooks!");
    println!("Hooks will:");
    println!("  - Log user messages and tool calls to conversation.log");
    println!("  - Display tool calls with emoji");
    println!("  - Block dangerous tools (rm, delete) before execution");

    Ok(())
}
