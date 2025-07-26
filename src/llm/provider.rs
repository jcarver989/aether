use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_stream::Stream;
use std::pin::Pin;

use super::openrouter::OpenRouterProvider;
use super::ollama::OllamaProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatMessage {
    System { content: String },
    User { content: String },
    Assistant { content: String },
    Tool { tool_call_id: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub type ChatStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn complete_stream(&self, request: ChatRequest) -> Result<ChatStream>;
    fn get_model(&self) -> &str;
}

#[derive(Debug, Clone)]
pub enum ProviderConfig {
    OpenRouter { api_key: String, model: String },
    Ollama { base_url: Option<String>, model: String },
}

pub fn create_provider(config: ProviderConfig) -> Result<Box<dyn LlmProvider>> {
    match config {
        ProviderConfig::OpenRouter { api_key, model } => {
            let provider = OpenRouterProvider::new(api_key, model)?;
            Ok(Box::new(provider))
        },
        ProviderConfig::Ollama { base_url, model } => {
            let provider = OllamaProvider::new(base_url, model)?;
            Ok(Box::new(provider))
        },
    }
}

pub fn create_provider_from_env() -> Result<Box<dyn LlmProvider>> {
    let provider_type = std::env::var("DEFAULT_PROVIDER").unwrap_or_else(|_| "openrouter".to_string());
    let model = std::env::var("DEFAULT_MODEL").unwrap_or_else(|_| {
        match provider_type.as_str() {
            "ollama" => "llama2".to_string(),
            _ => "anthropic/claude-3-sonnet".to_string(),
        }
    });

    match provider_type.as_str() {
        "openrouter" => {
            let api_key = std::env::var("OPENROUTER_API_KEY")
                .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY environment variable not set"))?;
            create_provider(ProviderConfig::OpenRouter { api_key, model })
        },
        "ollama" => {
            let base_url = std::env::var("OLLAMA_BASE_URL").ok();
            create_provider(ProviderConfig::Ollama { base_url, model })
        },
        _ => Err(anyhow::anyhow!("Unknown provider type: {}", provider_type)),
    }
}