use clap::Parser;
use cli::Cli;
use color_eyre::Result;
use config::Config;

use crate::{app::App, agent::Agent, mcp::{McpClient, registry::ToolRegistry}};

mod action;
mod agent;
mod app;
mod cli;
mod components;
mod config;
mod errors;
mod llm;
mod logging;
mod mcp;
mod mcp_config;
mod theme;
mod tui;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    crate::errors::init()?;
    crate::logging::init()?;

    let args = Cli::parse();
    
    // Load configuration to determine provider type
    let config = Config::with_cli_args(Some(&args))?;
    
    // Initialize MCP client and connect to servers
    let mut mcp_client = McpClient::new();
    
    // Connect to MCP servers from config
    for (name, server_config) in &config.config.mcp.servers {
        match mcp_client.connect_server(name.clone(), server_config.clone()).await {
            Ok(()) => println!("Connected to MCP server: {}", name),
            Err(e) => eprintln!("Failed to connect to MCP server {}: {}", name, e),
        }
    }
    
    // Discover tools
    let tool_registry = if mcp_client.discover_tools().await.is_ok() {
        mcp_client.get_tool_registry()
    } else {
        ToolRegistry::new()
    };
    
    // Create the appropriate provider and agent based on configuration
    match config.config.llm.provider {
        crate::config::ProviderType::OpenRouter => {
            let api_key = config.config.llm
                .openrouter_api_key
                .as_ref()
                .ok_or_else(|| color_eyre::Report::msg("OpenRouter API key not found"))?;
            
            let provider = crate::llm::openrouter::OpenRouterProvider::new(
                api_key.clone(), 
                config.config.llm.model.clone()
            ).map_err(|e| {
                color_eyre::Report::msg(format!("Failed to create OpenRouter provider: {}", e))
            })?;
            
            let agent = Agent::new(provider, tool_registry, config.config.agent_context.clone());
            let mut app = App::new(&args, agent)?;
            app.run().await?;
        }
        crate::config::ProviderType::Ollama => {
            let provider = crate::llm::ollama::OllamaProvider::new(
                Some(config.config.llm.ollama_base_url.clone()),
                config.config.llm.model.clone(),
            ).map_err(|e| {
                color_eyre::Report::msg(format!("Failed to create Ollama provider: {}", e))
            })?;
            
            let agent = Agent::new(provider, tool_registry, config.config.agent_context.clone());
            let mut app = App::new(&args, agent)?;
            app.run().await?;
        }
    }
    
    Ok(())
}
