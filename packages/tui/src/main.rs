use std::time::SystemTime;

use aether_core::{
    agent::Agent, 
    llm::openrouter::OpenRouterProvider, 
    mcp::McpClient,
    tools::ToolRegistry, 
    types::{ChatMessage, IsoString},
};
use std::sync::Arc;

use crate::{
    app::App,
    chat::{ChatMessageBlock, ChatWindow},
};

pub mod app;
pub mod chat;
pub mod event;
pub mod theme;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let mut chat = ChatWindow::new();

    for i in 0..50 {
        let message = if i % 2 == 0 {
            ChatMessage::System {
                content: format!("Hello, world! {}", i),
                timestamp: IsoString::now(),
            }
        } else {
            ChatMessage::User {
                content: format!("Hello, world! {}", i),
                timestamp: IsoString::now(),
            }
        };

        let block = ChatMessageBlock::new(message);
        chat.add_message(block);
    }

    let llm_provider =
        OpenRouterProvider::new("sk-or-v1-1234567890".to_string(), "gpt-4o-mini".to_string())?;

    // Create agent with empty tool registry
    let mut agent = Agent::new(
        llm_provider,
        ToolRegistry::new(),
        Some("You are a helpful coding agent".to_string()),
    );

    // Set up MCP client (this demonstrates the new architecture)
    // In a real app, you'd configure this from a config file
    let mut mcp_client = McpClient::new();
    // Example: connect to an MCP server
    // mcp_client.connect_server("example".to_string(), McpServerConfig::Http { 
    //     url: "http://localhost:3000/mcp".to_string(), 
    //     headers: std::collections::HashMap::new() 
    // }).await?;
    
    agent.set_mcp_client(Arc::new(mcp_client));
    agent.register_mcp_tools().await?;

    let result = App::new(agent, chat).run(terminal).await;
    ratatui::restore();
    result
}
