use async_openai::{
    Client,
    config::OpenAIConfig,
};
use color_eyre::Result;
use serde_json::json;
use tokio_stream::{Stream, StreamExt};
use async_stream;

use super::conversion::{convert_messages, convert_tools};
use super::openrouter_types::CustomChatCompletionStreamResponse;
use super::provider::{ChatRequest, LlmProvider};
use crate::types::StreamEvent;

pub struct OpenRouterProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: String, model: String) -> Result<Self> {
        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://openrouter.ai/api/v1");

        let client = Client::with_config(config);

        Ok(Self { client, model })
    }

}

impl LlmProvider for OpenRouterProvider {
    fn complete_stream_chunks(&self, request: ChatRequest) -> impl Stream<Item = Result<StreamEvent>> + Send {
        let client = self.client.clone();
        let model = self.model.clone();
        
        async_stream::stream! {
            let messages = convert_messages(request.messages);
            let tools = if request.tools.is_empty() {
                None
            } else {
                Some(convert_tools(request.tools))
            };

            let mut req = json!({
                "model": model.clone(),
                "messages": messages,
                "stream": true,
            });

            if let Some(tools) = tools {
                req["tools"] = json!(tools);
            }

            let stream = match client
                .chat()
                .create_stream_byot::<serde_json::Value, CustomChatCompletionStreamResponse>(req)
                .await {
                Ok(stream) => stream,
                Err(e) => {
                    yield Err(color_eyre::eyre::eyre!("OpenRouter API request failed: {}", e));
                    return;
                }
            };

            // Create a custom stream that properly handles tool calls
            let mut current_tool_id: Option<String> = None;
            let mut tool_args_buffer = String::new();

            let mut stream = Box::pin(stream);

            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        if let Some(choice) = response.choices.first() {
                            let delta = &choice.delta;

                            // Handle content
                            if let Some(content) = &delta.content {
                                // If we have a pending tool call and now we're getting content,
                                // complete the tool call first
                                if let Some(id) = current_tool_id.take() {
                                    yield Ok(StreamEvent::ToolCallComplete { id });
                                }
                                yield Ok(StreamEvent::Content { chunk: content.clone() });
                            }

                            // Handle tool calls
                            if let Some(tool_calls) = &delta.tool_calls {
                                for tool_call in tool_calls {
                                    if let Some(function) = &tool_call.function {
                                        // Tool call start
                                        if let Some(name) = &function.name {
                                            let id = tool_call.id.clone().unwrap_or_else(|| "tool_call_0".to_string());
                                            current_tool_id = Some(id.clone());
                                            tool_args_buffer.clear();
                                            yield Ok(StreamEvent::ToolCallStart {
                                                id,
                                                name: name.clone(),
                                            });
                                        }

                                        // Tool call arguments
                                        if let Some(arguments) = &function.arguments {
                                            if let Some(id) = &current_tool_id {
                                                tool_args_buffer.push_str(arguments);
                                                yield Ok(StreamEvent::ToolCallArgument {
                                                    id: id.clone(),
                                                    chunk: arguments.to_string(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }

                            if let Some(finish_reason) = &choice.finish_reason {
                                if format!("{finish_reason:?}").contains("tool_calls") {
                                    if let Some(id) = current_tool_id.take() {
                                        yield Ok(StreamEvent::ToolCallComplete { id });
                                    }
                                }
                                yield Ok(StreamEvent::Done);
                            }
                        } else {
                            yield Ok(StreamEvent::Done);
                        }
                    },
                    Err(e) => {
                        yield Err(color_eyre::eyre::eyre!("Stream error: {}", e));
                    }
                }
            }
        }
    }
}
