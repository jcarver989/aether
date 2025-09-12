use aether_core::agent::{Agent, AgentMessage::*};
use aether_core::llm::local::LocalLlmProvider;
use futures::pin_mut;
use std::env;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get user prompt from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <your question about the code>", args[0]);
        println!(
            "Example: {} \"Find all async functions in the agent module\"",
            args[0]
        );
        println!(
            "Example: {} \"Show me error handling patterns in this codebase\"",
            args[0]
        );
        return Ok(());
    }

    let user_prompt = args[1..].join(" ");
    let llm = LocalLlmProvider::new_llama_cpp()?;
    let system_prompt = Some(
        "You are a helpful code search and analysis assistant. You have access to powerful code search tools that can help you find patterns, functions, files, and analyze codebases. When users ask questions about code, use the available tools to search and provide detailed, helpful answers.".to_string()
    );

    let mut agent = Agent::new(llm, system_prompt).with_coding_tools().await?;
    let result_stream = agent.send_message(&user_prompt).await;

    pin_mut!(result_stream);

    println!("🤖 AI Agent Response:");
    let mut active_tool_calls: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    while let Some(event) = result_stream.next().await {
        match event {
            Message {
                chunk, is_complete, ..
            } => {
                if is_complete {
                    println!(); // New line when message is complete
                } else {
                    print!("{}", chunk);
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                }
            }

            ToolCall {
                tool_call_id,
                name,
                result,
                is_complete,
                ..
            } => {
                if is_complete {
                    let tool_name = active_tool_calls
                        .get(&tool_call_id)
                        .map(|(name, _)| name.clone())
                        .unwrap_or(name);

                    if let Some(result) = result {
                        println!("✅ Tool call '{}' completed", tool_name);
                        if result.len() > 100 {
                            println!("   Result: {}...", &result[..97]);
                        } else {
                            println!("   Result: {}", result);
                        }
                    }
                    active_tool_calls.remove(&tool_call_id);
                } else if !name.is_empty() {
                    println!("🔧 Tool call '{}' started", name);
                    active_tool_calls.insert(tool_call_id, (name, String::new()));
                }
            }

            Error { message } => {
                eprintln!("❌ Error: {}", message);
            }
        }
    }

    println!("\n🎉 Analysis complete!");

    Ok(())
}
