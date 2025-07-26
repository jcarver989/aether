use anyhow::Result;
use async_openai::{Client, config::OpenAIConfig};
use async_trait::async_trait;
use tokio::sync::mpsc;

use super::provider::{LlmProvider, Message, ToolCall};

pub struct OpenRouterClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenRouterClient {
    pub fn new(api_key: String, model: String) -> Result<Self> {
        todo!("Initialize OpenRouter client with API key and model")
    }
}

#[async_trait]
impl LlmProvider for OpenRouterClient {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Vec<serde_json::Value>,
        response_tx: mpsc::Sender<String>,
    ) -> Result<Vec<ToolCall>> {
        todo!("Implement OpenRouter chat completion with streaming")
    }
}