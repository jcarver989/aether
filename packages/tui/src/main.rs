use std::time::SystemTime;

use aether_core::{
    agent::Agent, llm::openrouter::OpenRouterProvider, tools::ToolRegistry, types::ChatMessage,
};

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
                timestamp: SystemTime::now(),
            }
        } else {
            ChatMessage::User {
                content: format!("Hello, world! {}", i),
                timestamp: SystemTime::now(),
            }
        };

        let block = ChatMessageBlock::new(message);
        chat.add_message(block);
    }

    let llm_provider =
        OpenRouterProvider::new("sk-or-v1-1234567890".to_string(), "gpt-4o-mini".to_string())?;

    let agent = Agent::new(
        llm_provider,
        ToolRegistry::new(),
        Some("You are a helpful coding agent".to_string()),
    );

    let result = App::new(agent, chat).run(terminal).await;
    ratatui::restore();
    result
}
