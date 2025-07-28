use anyhow::Result;
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
use async_trait::async_trait;
use tokio_stream::StreamExt;
use tracing::debug;

use super::provider::{
    ChatMessage, ChatRequest, LlmProvider, StreamChunk, StreamChunkStream, ToolDefinition,
};

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
                    parameters: Some(tool.parameters),
                    strict: Some(false),
                },
            })
            .collect()
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    async fn complete_stream_chunks(&self, request: ChatRequest) -> Result<StreamChunkStream> {
        let messages = self.convert_messages(request.messages);
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

        let stream = self.client.chat().create_stream(req).await?;

        // Create a custom stream that properly handles tool calls
        let mapped_stream = async_stream::stream! {
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
                                    yield Ok(StreamChunk::ToolCallComplete { id });
                                }
                                yield Ok(StreamChunk::Content(content.clone()));
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
                                            yield Ok(StreamChunk::ToolCallStart {
                                                id,
                                                name: name.clone(),
                                            });
                                        }

                                        // Tool call arguments
                                        if let Some(arguments) = &function.arguments {
                                            if let Some(id) = &current_tool_id {
                                                tool_args_buffer.push_str(arguments);
                                                yield Ok(StreamChunk::ToolCallArgument {
                                                    id: id.clone(),
                                                    argument: arguments.to_string(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }

                            // Handle finish reason
                            if let Some(finish_reason) = &choice.finish_reason {
                                debug!("Stream: Got finish_reason: {:?}, current_tool_id: {:?}", finish_reason, current_tool_id);

                                // Complete any pending tool call
                                if format!("{finish_reason:?}").contains("tool_calls") {
                                    if let Some(id) = current_tool_id.take() {
                                        yield Ok(StreamChunk::ToolCallComplete { id });
                                    }
                                }

                                // Send Done when stream is complete
                                yield Ok(StreamChunk::Done);
                            }
                        } else {
                            yield Ok(StreamChunk::Done);
                        }
                    },
                    Err(e) => {
                        yield Err(anyhow::anyhow!("Stream error: {}", e));
                    }
                }
            }
        };

        Ok(Box::pin(mapped_stream))
    }
}
