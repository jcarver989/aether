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

    #[arg(long, env = "DEFAULT_MODEL")]
    model: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let config = config::Config::load()?;

    // Read AGENT.md if present
    let agent_context = std::fs::read_to_string("AGENT.md").ok();

    // Initialize MCP client
    let mut mcp_client = mcp::McpClient::new();
    for (name, server_config) in config.mcp_servers {
        mcp_client.connect_server(name, server_config).await?;
    }
    mcp_client.discover_tools().await?;

    // Initialize LLM provider
    let llm_provider: Box<dyn llm::LlmProvider> = match config.provider {
        config::ProviderType::OpenRouter => {
            let api_key = config.openrouter_api_key
                .expect("OPENROUTER_API_KEY environment variable not set");
            let model = args.model.unwrap_or(config.model);
            Box::new(llm::openrouter::OpenRouterClient::new(api_key, model)?)
        }
        config::ProviderType::Ollama => {
            let model = args.model.unwrap_or(config.model);
            Box::new(llm::ollama::OllamaClient::new(config.ollama_base_url, model)?)
        }
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create and run app
    let mut app = ui::App::new(llm_provider, mcp_client);
    let res = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}
