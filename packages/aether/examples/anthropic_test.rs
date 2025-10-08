use aether::llm::Result;
use aether::llm::anthropic::AnthropicProvider;
use aether::llm::provider::{Context, StreamingModelProvider};
use aether::types::{ChatMessage, IsoString, LlmResponse, ToolDefinition};
use clap::Parser;
use futures::StreamExt;
use serde_json::json;
use std::io::{self, Write};

#[derive(Parser)]
#[command(author, version, about = "Test Anthropic provider integration")]
struct Args {
    /// The message to send to Claude
    #[arg(
        short,
        long,
        default_value = "Hello, Claude! Can you help me write a simple Rust function?"
    )]
    prompt: String,

    /// Disable prompt caching (enabled by default)
    #[arg(long)]
    no_cache: bool,

    /// Claude model to use
    #[arg(short = 'm', long, default_value = "claude-3-5-sonnet-20241022")]
    model: String,

    /// Maximum tokens
    #[arg(long, default_value = "1000")]
    max_tokens: u32,

    /// Temperature (0.0 to 1.0)
    #[arg(long, default_value = "0.7")]
    temperature: f32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("🤖 Testing Anthropic Provider");
    println!("Model: {}", args.model);
    println!("Message: {}", args.prompt);
    println!(
        "Caching: {}",
        if args.no_cache { "disabled" } else { "enabled" }
    );
    println!("Temperature: {}", args.temperature);
    println!("Max tokens: {}", args.max_tokens);
    println!("{}", "=".repeat(50));

    let provider = AnthropicProvider::default()?
        .with_model(&args.model)
        .with_temperature(args.temperature)
        .with_max_tokens(args.max_tokens);

    let provider = if args.no_cache {
        provider.with_prompt_caching(false)
    } else {
        provider // Caching is enabled by default
    };

    // Prepare context
    let messages = vec![
        ChatMessage::System {
            content:
                "You are a helpful AI assistant. Be concise but informative in your responses."
                    .to_string(),
            timestamp: IsoString::now(),
        },
        ChatMessage::User {
            content: args.prompt,
            timestamp: IsoString::now(),
        },
    ];

    let tools = vec![
        ToolDefinition {
            name: "search_web".to_string(),
            description: "Search the web for current information".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    }
                },
                "required": ["query"]
            })
            .to_string(),
            server: None,
        },
        ToolDefinition {
            name: "calculate".to_string(),
            description: "Perform mathematical calculations".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "Mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            })
            .to_string(),
            server: None,
        },
    ];
    let context = Context::new(messages, tools);

    // Stream the response
    let stream = provider.stream_response(&context);
    let mut stream = Box::pin(stream);

    print!("🔄 Streaming response: ");
    io::stdout().flush().unwrap();

    let mut current_tool_call = None;
    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        match result? {
            LlmResponse::Start { message_id } => {
                println!("\n✅ Started (ID: {})", message_id);
            }
            LlmResponse::Text { chunk } => {
                print!("{}", chunk);
                io::stdout().flush().unwrap();
                response_text.push_str(&chunk);
            }
            LlmResponse::ToolRequestStart { id, name } => {
                println!("\n🔧 Tool call started: {} ({})", name, id);
                current_tool_call = Some((id.clone(), name, String::new()));
            }
            LlmResponse::ToolRequestArg { id, chunk } => {
                if let Some((ref call_id, _, ref mut args)) = current_tool_call {
                    if call_id == &id {
                        args.push_str(&chunk);
                        print!(".");
                        io::stdout().flush().unwrap();
                    }
                }
            }
            LlmResponse::ToolRequestComplete { tool_call } => {
                println!(
                    "\n🔧 Tool call completed: {} with args: {}",
                    tool_call.name, tool_call.arguments
                );

                // Simulate tool execution (you would call actual tools here)
                let tool_result = match tool_call.name.as_str() {
                    "search_web" => {
                        "Search results: Found relevant information about Rust programming."
                    }
                    "calculate" => "Calculation result: 42",
                    _ => "Tool executed successfully",
                };

                println!("🔧 Tool result: {}", tool_result);
                current_tool_call = None;
            }
            LlmResponse::Done => {
                println!("\n✅ Stream completed");
                break;
            }
            LlmResponse::Error { message } => {
                println!("\n❌ Error: {}", message);
                break;
            }
        }
    }

    println!("\n{}", "=".repeat(50));
    println!("📊 Summary:");
    println!("Total response length: {} characters", response_text.len());
    println!("✅ Test completed successfully!");

    Ok(())
}
