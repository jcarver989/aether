use clap::Parser;
use cli::Cli;
use color_eyre::Result;
use config::Config;
use std::sync::Arc;
use tools::ToolRegistry;

use crate::{agent::Agent, app::App, mcp::McpClient};

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
mod tools;
mod tui;
mod types;

async fn run_with_openrouter_provider(
    args: Cli,
    config: Config,
    tool_registry: ToolRegistry,
) -> Result<()> {
    let api_key = config
        .config
        .llm
        .openrouter_api_key
        .as_ref()
        .ok_or_else(|| color_eyre::Report::msg("OpenRouter API key not found"))?;

    let provider = crate::llm::openrouter::OpenRouterProvider::new(
        api_key.clone(),
        config.config.llm.model.clone(),
    )
    .map_err(|e| color_eyre::Report::msg(format!("Failed to create OpenRouter provider: {e}")))?;

    let agent = Agent::new(provider, tool_registry, config.config.agent_context.clone());
    let mut app = App::new(&args, agent)?;
    app.run().await
}

async fn run_with_ollama_provider(
    args: Cli,
    config: Config,
    tool_registry: ToolRegistry,
) -> Result<()> {
    let provider = crate::llm::ollama::OllamaProvider::new(
        Some(config.config.llm.ollama_base_url.clone()),
        config.config.llm.model.clone(),
    )
    .map_err(|e| color_eyre::Report::msg(format!("Failed to create Ollama provider: {e}")))?;

    let agent = Agent::new(provider, tool_registry, config.config.agent_context.clone());
    let mut app = App::new(&args, agent)?;
    app.run().await
}

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
        match mcp_client
            .connect_server(name.clone(), server_config.clone())
            .await
        {
            Ok(()) => println!("Connected to MCP server: {name}"),
            Err(e) => eprintln!("Failed to connect to MCP server {name}: {e}"),
        }
    }

    // Discover tools and create registry
    let tool_registry = match mcp_client.discover_tools().await {
        Ok(discovered_tools) => {
            let mut registry = ToolRegistry::new();
            // Register all discovered tools
            for (server_name, tool) in discovered_tools {
                registry.register_tool(server_name, tool);
            }
            // Wrap MCP client in Arc after discovery
            let mcp_client_arc = Arc::new(mcp_client);
            // Set the MCP client in the registry for tool execution
            registry.set_mcp_client(mcp_client_arc);
            registry
        }
        Err(_) => ToolRegistry::new(),
    };

    // Create the appropriate provider and agent based on configuration
    match config.config.llm.provider {
        crate::config::ProviderType::OpenRouter => {
            run_with_openrouter_provider(args, config, tool_registry).await?
        }
        crate::config::ProviderType::Ollama => {
            run_with_ollama_provider(args, config, tool_registry).await?
        }
    }

    Ok(())
}
