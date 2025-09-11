use async_openai::{Client, config::OpenAIConfig, types::CreateChatCompletionRequest};
use async_stream;
use color_eyre::Result;
use std::error::Error;
use tokio_stream::{Stream, StreamExt};
use tracing::{debug, error, info};

use super::mappers::{map_messages, mapp_tools};
use super::provider::{ChatRequest, LlmProvider};
use super::streaming::process_completion_stream;
use crate::types::StreamEvent;

pub enum LocalProvider {
    Ollama,
    LlamaCpp,
}

pub struct LocalLlmProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl LocalLlmProvider {
    pub fn new_ollama(model: &str) -> Result<Self> {
        Self::new("http://localhost:11434", model)
    }

    pub fn new_llama_cpp() -> Result<Self> {
        // Currently ignores model as LLama.cpp serves a single model per instance
        Self::new("http://localhost:8080", "")
    }

    pub fn new(base_url: &str, model: &str) -> Result<Self> {
        let base_url = base_url.to_string();

        // Ensure we have the correct base URL with /v1 for Ollama's OpenAI-compatible API
        let api_base = if base_url.ends_with("/v1") {
            base_url
        } else {
            format!("{}/v1", base_url)
        };

        info!(
            "Creating OllamaProvider with api_base: {}, model: {}",
            api_base, model
        );

        let config = OpenAIConfig::new()
            .with_api_key("dummy-key") // Ollama doesn't require auth but async-openai needs a key
            .with_api_base(api_base.clone());

        let client = Client::with_config(config);

        Ok(Self {
            client,
            model: model.to_string(),
        })
    }
}

impl LlmProvider for LocalLlmProvider {
    fn complete_stream_chunks(
        &self,
        request: ChatRequest,
    ) -> impl Stream<Item = Result<StreamEvent>> + Send {
        let client = self.client.clone();
        let model = self.model.clone();

        async_stream::stream! {
            debug!("Starting chat completion stream for model: {}", model);

            let messages = map_messages(request.messages);
            let message_count = messages.len();
            let tools = if request.tools.is_empty() {
                None
            } else {
                Some(mapp_tools(request.tools))
            };

            let req = CreateChatCompletionRequest {
                model: model.clone(),
                messages,
                tools,
                stream: Some(true),
                ..Default::default()
            };

            debug!(
                "Making request to Ollama API with model: {} and {} messages",
                model, message_count
            );

            let stream = match client.chat().create_stream(req).await {
                Ok(stream) => {
                    debug!("Successfully created stream from Ollama API");
                    stream
                }
                Err(e) => {
                    error!("Failed to create stream from Ollama API: {:?}", e);

                    // Check if it's a reqwest error with more details
                    if let Some(reqwest_err) =
                        e.source().and_then(|s| s.downcast_ref::<reqwest::Error>())
                    {
                        if let Some(url) = reqwest_err.url() {
                            error!("Request URL was: {}", url);
                        }
                        if let Some(status) = reqwest_err.status() {
                            error!("HTTP status: {}", status);
                        }
                    }

                    yield Err(color_eyre::eyre::eyre!("Ollama API request failed: {}", e));
                    return;
                }
            };

            // Use the shared streaming logic directly with standard types
            let mut shared_stream = Box::pin(process_completion_stream(stream));
            while let Some(result) = shared_stream.next().await {
                yield result;
            }
        }
    }
}
