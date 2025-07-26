use anyhow::Result;
use async_openai::{Client, config::OpenAIConfig};
use async_trait::async_trait;
use tokio::sync::mpsc;

use super::provider::{LlmProvider, Message, ToolCall};

pub struct OllamaClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Result<Self> {
        todo!("Initialize Ollama client with base URL and model")
    }
}

#[async_trait]
impl LlmProvider for OllamaClient {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Vec<serde_json::Value>,
        response_tx: mpsc::Sender<String>,
    ) -> Result<Vec<ToolCall>> {
        todo!("Implement Ollama chat completion with streaming")
    }
}