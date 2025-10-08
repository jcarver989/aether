use aether::{
    agent::{AgentMessage::*, UserMessage, agent},
    llm::{
        StreamingModelProvider,
        anthropic::AnthropicProvider,
        local::{llama_cpp::LlamaCppProvider, ollama::OllamaProvider},
        openrouter::OpenRouterProvider,
    },
    types::LlmProvider,
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

    #[arg(short = 'm', long = "model", help = "Model name to use")]
    model: String,

    #[arg(long = "provider", help = "LLM provider to use", value_enum)]
    provider: Option<LlmProvider>,
}

#[tokio::main]
pub async fn main() {
    let cli = Cli::parse();
    let prompt = match cli.prompt.clone() {
        Some(p) => p,
        None => {
            eprintln!("Error: --prompt is required");
            std::process::exit(1);
        }
    };

    match cli.provider {
        Some(LlmProvider::Anthropic) => {
            let provider = AnthropicProvider::default().unwrap().with_model(&cli.model);
            run_agent(provider, &cli, &prompt).await;
        }
        Some(LlmProvider::OpenRouter) => {
            let model = if cli.model.is_empty() {
                "anthropic/claude-3.5-sonnet"
            } else {
                &cli.model
            };
            let provider = OpenRouterProvider::default(model).unwrap();
            run_agent(provider, &cli, &prompt).await;
        }
        Some(LlmProvider::Ollama) => {
            let model = if cli.model.is_empty() {
                "llama3.2"
            } else {
                &cli.model
            };
            let provider = OllamaProvider::default(model);
            run_agent(provider, &cli, &prompt).await;
        }
        Some(LlmProvider::LlamaCpp) | None => {
            let provider = LlamaCppProvider::default();
            run_agent(provider, &cli, &prompt).await;
        }
    }
}

async fn run_agent<T: StreamingModelProvider + 'static>(provider: T, cli: &Cli, prompt: &str) {
    let mut agent_builder = agent(provider);

    if let Some(system) = &cli.system {
        agent_builder = agent_builder.system(system);
    }

    let mut agent = agent_builder.spawn().await.unwrap();

    // Send the prompt
    agent.send(UserMessage::text(prompt)).await.unwrap();

    while let Some(event) = agent.recv().await {
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
}
