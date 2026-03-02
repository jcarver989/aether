use aether_core::{
    core::{Prompt, agent},
    events::{AgentMessage, UserMessage},
    mcp::{McpSpawnResult, mcp},
};
use llm::providers::openrouter::OpenRouterProvider;

use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let llm = OpenRouterProvider::default("z-ai/glm-4.5-air")?;
    let McpSpawnResult {
        tool_definitions: tools,
        instructions: _,
        server_statuses: _,
        command_tx: mcp_tx,
        elicitation_rx: _,
        handle: _mcp_handle,
    } = mcp()
        .from_json_file("examples/mcp.json")
        .await?
        .spawn()
        .await?;

    let (tx, mut rx, _handle) = agent(llm)
        .system_prompt(Prompt::text(
            "You are a helpful assistant with access to web browsing tools via Playwright.",
        ))
        .tools(mcp_tx, tools)
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
                    print!("{chunk}");
                    io::stdout().flush().unwrap();
                } else {
                    println!();
                }
            }
            Some(ToolCall { request, .. }) => {
                println!("\nTool '{}' in progress", request.name);
            }
            Some(ToolResult { result, .. }) => {
                println!("\nTool '{}' completed successfully", result.name);
            }
            Some(ToolError { error, .. }) => {
                eprintln!("\nTool '{}' failed: {}", error.name, error.error);
            }
            Some(ToolProgress {
                request,
                progress,
                total,
                message,
            }) => {
                let msg = message
                    .as_ref()
                    .map(|m| format!("{m} "))
                    .unwrap_or_default();
                let total_str = total.map(|t| format!("/{t}")).unwrap_or_default();
                println!(
                    "\nTool '{}' progress: {}{}{}",
                    request.name, msg, progress, total_str
                );
            }
            Some(Done) => {
                println!("\nAgent finished");
                break;
            }
            Some(Error { message }) => {
                eprintln!("Error: {message}");
                break;
            }
            Some(Cancelled { .. }) => {
                println!("Cancelled");
                break;
            }
            Some(ContextCompactionStarted { message_count }) => {
                println!("Context compaction started: {} messages", message_count);
            }
            Some(ContextCompactionResult {
                messages_removed, ..
            }) => {
                println!("Context compacted: {} messages removed", messages_removed);
            }
            Some(ContextUsageUpdate {
                usage_ratio,
                tokens_used,
                context_limit,
            }) => match (usage_ratio, context_limit) {
                (Some(usage_ratio), Some(context_limit)) => {
                    println!(
                        "Context usage: {:.1}% ({}/{} tokens)",
                        usage_ratio * 100.0,
                        tokens_used,
                        context_limit
                    );
                }
                _ => {
                    println!("Context usage: unknown limit ({tokens_used} tokens used)");
                }
            },
            Some(AutoContinue {
                attempt,
                max_attempts,
            }) => {
                println!(
                    "Auto-continuing: attempt {}/{} (LLM stopped due to length)",
                    attempt, max_attempts
                );
            }
            Some(ModelSwitched { previous, new }) => {
                println!("Model switched: {} -> {}", previous, new);
            }
            Some(ContextCleared) => {
                println!("Context cleared");
            }
            Some(Thought { chunk, .. }) => {
                print!("{chunk}");
                io::stdout().flush().unwrap();
            }
            None => {
                println!("Channel closed");
                break;
            }
        }
    }

    Ok(())
}
