use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
        ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessage,
        ChatCompletionTool, ChatCompletionToolType, FunctionCall, FunctionObject,
    },
};
use color_eyre::Result;
use serde_json::json;
use tokio_stream::{Stream, StreamExt};
use async_stream;

use super::openrouter_types::CustomChatCompletionStreamResponse;
use super::provider::{ChatRequest, LlmProvider};
use crate::types::{ChatMessage, StreamEvent, ToolDefinition};

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

    fn convert_messages(messages: Vec<ChatMessage>) -> Vec<ChatCompletionRequestMessage> {
        messages
            .into_iter()
            .flat_map(|msg| match msg {
                ChatMessage::System { content, .. } => Some(ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessage {
                        content: content.into(),
                        name: None,
                    },
                )),
                ChatMessage::User { content, .. } => Some(ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessage {
                        content: content.into(),
                        name: None,
                    },
                )),
                ChatMessage::Assistant {
                    content,
                    tool_calls,
                    ..
                } => {
                    let openai_tool_calls: Vec<_> = tool_calls
                        .iter()
                        .map(|call| ChatCompletionMessageToolCall {
                            id: call.id.clone(),
                            r#type: ChatCompletionToolType::Function,
                            function: FunctionCall {
                                name: call.name.clone(),
                                arguments: call.arguments.to_string(),
                            },
                        })
                        .collect();

                    let tool_calls = if openai_tool_calls.is_empty() {
                        None
                    } else {
                        Some(openai_tool_calls)
                    };

                    Some(ChatCompletionRequestMessage::Assistant(
                        ChatCompletionRequestAssistantMessage {
                            content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                                content,
                            )),
                            name: None,
                            tool_calls,
                            audio: None,
                            refusal: None,
                            #[allow(deprecated)]
                            function_call: None,
                        },
                    ))
                }
                ChatMessage::ToolCallResult {
                    tool_call_id,
                    content,
                    ..
                } => Some(ChatCompletionRequestMessage::Tool(
                    ChatCompletionRequestToolMessage {
                        content: ChatCompletionRequestToolMessageContent::Text(content),
                        tool_call_id,
                    },
                )),

                ChatMessage::AssistantStreaming { .. } | ChatMessage::Error { .. } => None,
            })
            .collect()
    }

    fn convert_tools(tools: Vec<ToolDefinition>) -> Vec<ChatCompletionTool> {
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

impl LlmProvider for OpenRouterProvider {
    fn complete_stream_chunks(&self, request: ChatRequest) -> impl Stream<Item = Result<StreamEvent>> + Send {
        let client = self.client.clone();
        let model = self.model.clone();
        
        async_stream::stream! {
            let messages = Self::convert_messages(request.messages);
            let tools = if request.tools.is_empty() {
                None
            } else {
                Some(Self::convert_tools(request.tools))
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
