use aether::{
    agent::{AgentMessage::*, Prompt, UserMessage, agent},
    llm::{StreamingModelProvider, parser::ModelProviderParser},
    mcp::{McpConfigParser, McpError, McpServerConfig, mcp_builder::mcp},
};
use clap::Parser;
use rmcp::model::{CreateElicitationResult, ElicitationAction};

#[derive(Parser)]
#[command(name = "aether-cli")]
#[command(about = "A CLI for the Aether AI assistant")]
struct Cli {
    #[arg(short = 'p', long = "prompt", help = "The LLM's prompt")]
    prompt: Option<String>,

    #[arg(short = 's', long = "system", help = "The LLM's system prompt")]
    system: Option<String>,

    #[arg(
        short = 'm',
        long = "model",
        help = "Model spec (e.g., 'anthropic:claude-3.5-sonnet', 'ollama:llama3.2', or 'llamacpp')"
    )]
    model: String,
}

#[tokio::main]
pub async fn main() {
    let cli = Cli::parse();

    let llm = match ModelProviderParser::default().parse(&cli.model) {
        Ok(llm) => llm,
        Err(e) => {
            eprintln!("Error parsing model spec '{}': {}", cli.model, e);
            std::process::exit(1);
        }
    };

    let system_prompt = match cli.system.clone().or(Prompt::agents_md().build().ok()) {
        Some(p) => p,
        None => {
            eprintln!("Error: AGENTS.md or --system is required");
            std::process::exit(1);
        }
    };

    let prompt = match cli.prompt.clone() {
        Some(p) => p,
        None => {
            eprintln!("Error: --prompt is required");
            std::process::exit(1);
        }
    };

    let mcp_configs = match McpConfigParser::new().parse_json_file("mcp.json") {
        Ok(conifgs) => conifgs,
        Err(_) => {
            println!("No MCP servers loaded");
            Vec::new()
        }
    };

    match run_agent(llm, &system_prompt, &prompt, mcp_configs).await {
        Ok(_) => println!("Done!"),
        Err(e) => println!("Error: {}", e),
    };
}

async fn run_agent(
    llm: Box<dyn StreamingModelProvider>,
    system: &str,
    prompt: &str,
    mcp_configs: Vec<McpServerConfig>,
) -> Result<(), McpError> {
    let (tools, mcp_tx, _mcp_handle) = mcp().add(mcp_configs).spawn().await?;

    let (tx, mut rx, _handle) = agent(llm)
        .system(system)
        .mcp_tools(mcp_tx, tools)
        .spawn()
        .await
        .unwrap();

    tx.send(UserMessage::text(prompt)).await.unwrap();
    while let Some(event) = rx.recv().await {
        match event {
            Text {
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
                name, is_complete, ..
            } => {
                if is_complete {
                    println!("Tool call '{}' completed", name);
                } else {
                    println!("Tool call '{}' started", name);
                }
            }

            Error { message } => {
                eprintln!("Error: {}", message);
            }

            Cancelled { message } => {
                eprintln!("Cancelled: {}", message);
            }

            ElicitationRequest {
                request_id,
                request,
            } => {
                println!("Elicitation request ({}): {}", request_id, request.message);

                let _result = CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                };
            }

            Done => {
                println!("Agent task completed");
                break;
            }
        }
    }

    Ok(())
}
