use async_openai::{Client, config::Config, types::CreateChatCompletionRequest};
use async_stream;
use std::error::Error;
use tokio_stream::StreamExt;
use tracing::{debug, error};

use crate::llm::{
    Context, LlmResponseStream, ModelProvider,
    openai::{
        mappers::{map_messages, map_tools},
        streaming::process_completion_stream,
    },
};

/// A Provider that's compatible with OpenAI's chat completion API
/// Other providers (e.g. Ollama, Llama.cpp etc) that are "OpenAI compatible" should implement this trait as well
pub trait OpenAiChatProvider {
    type Config: Config + Clone + 'static;

    fn client(&self) -> &Client<Self::Config>;
    fn model(&self) -> &str;
    fn provider_name(&self) -> &str;
}

impl<T: OpenAiChatProvider + Send + Sync> ModelProvider for T {
    fn stream_response(&self, request: Context) -> LlmResponseStream {
        let client = self.client().clone();
        let model = self.model().to_string();

        Box::pin(async_stream::stream! {
            debug!("Starting chat completion stream for model: {}", model);

            let messages = map_messages(request.messages);
            let message_count = messages.len();
            let tools = if request.tools.is_empty() {
                None
            } else {
                Some(map_tools(request.tools))
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

            let mut shared_stream = Box::pin(process_completion_stream(stream));
            while let Some(result) = shared_stream.next().await {
                yield result;
            }
        })
    }

    fn display_name(&self) -> String {
        let model = self.model();
        if model.is_empty() {
            self.provider_name().to_string()
        } else {
            format!("{} ({})", self.provider_name(), model)
        }
    }
}
