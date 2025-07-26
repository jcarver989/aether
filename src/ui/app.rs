use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    backend::Backend,
    Terminal,
};
use tokio::sync::mpsc;

use crate::llm::LlmProvider;
use crate::mcp::McpClient;

pub struct App {
    messages: Vec<String>,
    input: String,
    llm_provider: Box<dyn LlmProvider>,
    mcp_client: McpClient,
}

impl App {
    pub fn new(llm_provider: Box<dyn LlmProvider>, mcp_client: McpClient) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            llm_provider,
            mcp_client,
        }
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        break;
                    }
                    KeyCode::Enter => {
                        self.handle_input().await?;
                    }
                    KeyCode::Char(c) => {
                        self.input.push(c);
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn render(&self, f: &mut ratatui::Frame) {
        todo!("Render UI components")
    }

    async fn handle_input(&mut self) -> Result<()> {
        todo!("Process user input and send to LLM")
    }
}