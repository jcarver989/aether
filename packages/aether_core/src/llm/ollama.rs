use async_openai::{
    Client,
    config::OpenAIConfig,
    types::CreateChatCompletionRequest,
};
use async_stream;
use color_eyre::Result;
use std::error::Error;
use tokio_stream::{Stream, StreamExt};
use tracing::{debug, error, info};

use super::conversion::{convert_messages, convert_tools};
use super::provider::{ChatRequest, LlmProvider};
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

            let messages = convert_messages(request.messages);
            let message_count = messages.len();
            let tools = if request.tools.is_empty() {
                None
            } else {
                Some(convert_tools(request.tools))
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

            // Emit start event with a message ID
            let message_id = uuid::Uuid::new_v4().to_string();
            yield Ok(StreamEvent::Start { message_id: message_id.clone() });

            // Create a stateful stream to track tool calls
            let mut current_tool_id: Option<String> = None;
            let mut stream = Box::pin(stream);

            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        if let Some(choice) = response.choices.first() {
                            let delta = &choice.delta;

                            // Handle content
                            if let Some(content) = &delta.content {
                                if !content.is_empty() {
                                    // If we have a pending tool call and now we're getting content,
                                    // complete the tool call first
                                    if let Some(id) = current_tool_id.take() {
                                        yield Ok(StreamEvent::ToolCallComplete { id });
                                    }
                                    yield Ok(StreamEvent::Content { chunk: content.clone() });
                                }
                            }

                            // Handle tool calls
                            if let Some(tool_calls) = &delta.tool_calls {
                                for tool_call in tool_calls {
                                    if let Some(function) = &tool_call.function {
                                        // Tool call start
                                        if let Some(name) = &function.name {
                                            let id = tool_call.id.clone().unwrap_or_else(|| "tool_call_0".to_string());
                                            current_tool_id = Some(id.clone());
                                            yield Ok(StreamEvent::ToolCallStart {
                                                id,
                                                name: name.clone(),
                                            });
                                        }

                                        // Tool call arguments
                                        if let Some(arguments) = &function.arguments {
                                            if !arguments.is_empty() {
                                                if let Some(id) = &current_tool_id {
                                                    yield Ok(StreamEvent::ToolCallArgument {
                                                        id: id.clone(),
                                                        chunk: arguments.clone(),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Handle finish reason - this indicates stream completion
                            if let Some(finish_reason) = &choice.finish_reason {
                                let finish_reason_str = format!("{finish_reason:?}");
                                debug!("Received finish reason: {}", finish_reason_str);

                                // Complete any pending tool call before ending
                                if let Some(id) = current_tool_id.take() {
                                    yield Ok(StreamEvent::ToolCallComplete { id });
                                }

                                // End the stream for any finish reason
                                yield Ok(StreamEvent::Done);
                                break;
                            }
                        } else {
                            // No choices means stream is done
                            debug!("No choices in response, ending stream");
                            if let Some(id) = current_tool_id.take() {
                                yield Ok(StreamEvent::ToolCallComplete { id });
                            }
                            yield Ok(StreamEvent::Done);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Stream error: {}", e);
                        yield Err(color_eyre::eyre::eyre!("Stream error: {}", e));
                        break;
                    }
                }
            }
        }
    }
}
