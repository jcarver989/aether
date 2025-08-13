use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
        ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessage,
        ChatCompletionTool, ChatCompletionToolType, CreateChatCompletionRequest, FunctionCall,
        FunctionObject,
    },
};
use async_stream;
use async_trait::async_trait;
use color_eyre::Result;
use tokio_stream::StreamExt;
use tracing::{debug, info, error};
use std::error::Error;

use super::provider::{
    ChatMessage, ChatRequest, LlmProvider, StreamChunk, StreamChunkStream, ToolDefinition,
};

pub struct OllamaProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: Option<String>, model: String) -> Result<Self> {
        let base_url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
        
        // Ensure we have the correct base URL with /v1 for Ollama's OpenAI-compatible API
        let api_base = if base_url.ends_with("/v1") {
            base_url
        } else {
            format!("{}/v1", base_url)
        };
        
        info!("Creating OllamaProvider with api_base: {}, model: {}", api_base, model);

        let config = OpenAIConfig::new()
            .with_api_key("dummy-key") // Ollama doesn't require auth but async-openai needs a key
            .with_api_base(api_base.clone());

        let client = Client::with_config(config);

        Ok(Self { client, model })
    }

    fn convert_messages(&self, messages: Vec<ChatMessage>) -> Vec<ChatCompletionRequestMessage> {
        messages
            .into_iter()
            .map(|msg| match msg {
                ChatMessage::System { content } => {
                    ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                        content: content.into(),
                        name: None,
                    })
                }
                ChatMessage::User { content } => {
                    ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                        content: content.into(),
                        name: None,
                    })
                }
                ChatMessage::Assistant {
                    content,
                    tool_calls,
                } => {
                    let openai_tool_calls = tool_calls.as_ref().map(|calls| {
                        calls
                            .iter()
                            .map(|call| ChatCompletionMessageToolCall {
                                id: call.id.clone(),
                                r#type: ChatCompletionToolType::Function,
                                function: FunctionCall {
                                    name: call.name.clone(),
                                    arguments: call.arguments.to_string(),
                                },
                            })
                            .collect()
                    });

                    ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                        content: Some(ChatCompletionRequestAssistantMessageContent::Text(content)),
                        name: None,
                        tool_calls: openai_tool_calls,
                        audio: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    })
                }
                ChatMessage::Tool {
                    tool_call_id,
                    content,
                } => ChatCompletionRequestMessage::Tool(ChatCompletionRequestToolMessage {
                    content: ChatCompletionRequestToolMessageContent::Text(content),
                    tool_call_id,
                }),
            })
            .collect()
    }

    fn convert_tools(&self, tools: Vec<ToolDefinition>) -> Vec<ChatCompletionTool> {
        tools
            .into_iter()
            .map(|tool| ChatCompletionTool {
                r#type: ChatCompletionToolType::Function,
                function: FunctionObject {
                    name: tool.name,
                    description: Some(tool.description),
                    parameters: Some(serde_json::from_str(&tool.parameters).unwrap_or_default()),
                    strict: Some(false),
                },
            })
            .collect()
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn complete_stream_chunks(&self, request: ChatRequest) -> Result<StreamChunkStream> {
        debug!("Starting chat completion stream for model: {}", self.model);
        
        let messages = self.convert_messages(request.messages);
        let message_count = messages.len();
        let tools = if request.tools.is_empty() {
            None
        } else {
            Some(self.convert_tools(request.tools))
        };

        let req = CreateChatCompletionRequest {
            model: self.model.clone(),
            messages,
            tools,
            temperature: request.temperature,
            stream: Some(true),
            ..Default::default()
        };

        debug!("Making request to Ollama API with model: {} and {} messages", self.model, message_count);
        let stream = match self.client.chat().create_stream(req).await {
            Ok(stream) => {
                debug!("Successfully created stream from Ollama API");
                stream
            }
            Err(e) => {
                error!("Failed to create stream from Ollama API: {:?}", e);
                
                // Check if it's a reqwest error with more details
                if let Some(reqwest_err) = e.source().and_then(|s| s.downcast_ref::<reqwest::Error>()) {
                    if let Some(url) = reqwest_err.url() {
                        error!("Request URL was: {}", url);
                    }
                    if let Some(status) = reqwest_err.status() {
                        error!("HTTP status: {}", status);
                    }
                }
                
                return Err(color_eyre::eyre::eyre!("Ollama API request failed: {}", e));
            }
        };

        // Create a stateful stream to track tool calls
        let mapped_stream = async_stream::stream! {
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
                                        yield Ok(StreamChunk::ToolCallComplete { id });
                                    }
                                    yield Ok(StreamChunk::Content { content: content.clone() });
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
                                            yield Ok(StreamChunk::ToolCallStart {
                                                id,
                                                name: name.clone(),
                                            });
                                        }

                                        // Tool call arguments
                                        if let Some(arguments) = &function.arguments {
                                            if !arguments.is_empty() {
                                                if let Some(id) = &current_tool_id {
                                                    yield Ok(StreamChunk::ToolCallArgument {
                                                        id: id.clone(),
                                                        argument: arguments.clone(),
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
                                    yield Ok(StreamChunk::ToolCallComplete { id });
                                }
                                
                                // End the stream for any finish reason
                                yield Ok(StreamChunk::Done);
                                break;
                            }
                        } else {
                            // No choices means stream is done
                            debug!("No choices in response, ending stream");
                            if let Some(id) = current_tool_id.take() {
                                yield Ok(StreamChunk::ToolCallComplete { id });
                            }
                            yield Ok(StreamChunk::Done);
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
        };

        Ok(Box::pin(mapped_stream))
    }
}
