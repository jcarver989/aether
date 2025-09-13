use aether::{
    agent::{AgentMessage, UserMessage, agent},
    llm::local::DefaultModelProvider,
};
use clap::Parser;
use futures::pin_mut;
use tokio_stream::StreamExt;

#[derive(Parser)]
#[command(name = "aether-cli")]
#[command(about = "A CLI for the Aether AI assistant")]
struct Cli {
    #[arg(short = 'p', long = "prompt", help = "The LLM's prompt")]
    prompt: Option<String>,

    #[arg(short = 's', long = "system", help = "The LLM's system prompt")]
    system: Option<String>,

    #[arg(
        short = 'u',
        long = "url",
        help = "HTTP endpoint URL for the LLM provider. Defaults to http://localhost:8080 (LLama.cpp server's default port)",
        default_value = "http://localhost:8080"
    )]
    url: String,

    #[arg(short = 'k', long = "api-key", help = "API key for the LLM provider")]
    api_key: Option<String>,

    #[arg(
        short = 'm',
        long = "model",
        help = "Model name to use",
        default_value = ""
    )]
    model: String,
}

#[tokio::main]
pub async fn main() {
    let cli = Cli::parse();
    let prompt = cli.prompt.unwrap();

    let provider = DefaultModelProvider::new(&cli.url, &cli.model, cli.api_key).unwrap();
    let mut agent = agent(provider)
        .system(&cli.system.unwrap_or_default())
        .build()
        .await
        .unwrap();

    let (result_stream, _cancel_token) = agent.send(UserMessage::text(&prompt)).await;
    pin_mut!(result_stream);

    while let Some(event) = result_stream.next().await {
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
}
