mod config;
mod llm;
mod mcp;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, env = "DEFAULT_PROVIDER", default_value = "openrouter")]
    provider: String,

    #[arg(long, env = "DEFAULT_MODEL", default_value = "qwen/qwen3-coder")]
    model: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    println!("Loading configuration...");
    let config = config::Config::load()?;

    // Read AGENT.md if present
    println!("Checking for AGENT.md...");
    let agent_context = std::fs::read_to_string("AGENT.md").ok();
    if agent_context.is_some() {
        println!("Found AGENT.md");
    }

    // Initialize MCP client
    println!("Connecting to MCP servers...");
    let mut mcp_client = mcp::McpClient::new();
    for (name, server_config) in config.mcp_servers {
        println!("Connecting to MCP server: {}", name);
        mcp_client.connect_server(name, server_config).await?;
    }
    println!("Discovering tools...");
    mcp_client.discover_tools().await?;
    println!("Found {} tools", mcp_client.get_available_tools().len());

    // Initialize LLM provider
    println!("Initializing LLM provider: {:?}", config.provider);
    let llm_provider: Box<dyn llm::LlmProvider> = match config.provider {
        config::ProviderType::OpenRouter => {
            let api_key = config.openrouter_api_key
                .expect("OPENROUTER_API_KEY environment variable not set");
            let model = args.model.unwrap_or(config.model);
            println!("Using OpenRouter with model: {}", model);
            Box::new(llm::openrouter::OpenRouterProvider::new(api_key, model)?)
        }
        config::ProviderType::Ollama => {
            let model = args.model.unwrap_or(config.model);
            println!("Using Ollama with model: {}", model);
            Box::new(llm::ollama::OllamaProvider::new(Some(config.ollama_base_url), model)?)
        }
    };

    println!("Starting terminal UI...");

    // Run the terminal UI with proper cleanup
    run_terminal_ui(llm_provider, mcp_client, agent_context).await
}

async fn run_terminal_ui(
    llm_provider: Box<dyn llm::LlmProvider>,
    mcp_client: mcp::McpClient,
    agent_context: Option<String>,
) -> Result<()> {
    // Setup panic hook to restore terminal
    std::panic::set_hook(Box::new(|panic| {
        let _ = restore_terminal();
        eprintln!("Application panicked: {}", panic);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create and run app
    let mut app = ui::App::new(llm_provider, mcp_client).with_agent_context(agent_context);
    let res = app.run(&mut terminal).await;

    // Restore terminal
    restore_terminal()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}
