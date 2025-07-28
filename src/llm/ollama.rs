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

use super::provider::{
    ChatMessage, ChatRequest, LlmProvider, StreamChunk, StreamChunkStream, ToolDefinition,
};

pub struct OllamaProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: Option<String>, model: String) -> Result<Self> {
        let base_url = base_url.unwrap_or_else(|| "http://localhost:11434/v1".to_string());

        let config = OpenAIConfig::new()
            .with_api_key("dummy-key") // Ollama doesn't require auth but async-openai needs a key
            .with_api_base(base_url);

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
impl LlmProvider for OllamaProvider {
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

        let mapped_stream = stream.map(|result| {
            match result {
                Ok(response) => {
                    if let Some(choice) = response.choices.first() {
                        let delta = &choice.delta;

                        // Handle content
                        if let Some(content) = &delta.content {
                            return Ok(StreamChunk::Content(content.clone()));
                        }

                        // Handle tool calls
                        if let Some(tool_calls) = &delta.tool_calls {
                            for tool_call in tool_calls {
                                if let Some(function) = &tool_call.function {
                                    // Tool call start
                                    if let Some(name) = &function.name {
                                        return Ok(StreamChunk::ToolCallStart {
                                            id: tool_call.id.clone().unwrap_or_default(),
                                            name: name.clone(),
                                        });
                                    }

                                    // Tool call arguments
                                    if let Some(arguments) = &function.arguments {
                                        return Ok(StreamChunk::ToolCallArgument {
                                            id: tool_call.id.clone().unwrap_or_default(),
                                            argument: arguments.clone(),
                                        });
                                    }
                                }
                            }
                        }

                        // Handle finish reason for tool call completion
                        if let Some(finish_reason) = &choice.finish_reason {
                            if format!("{:?}", finish_reason).contains("tool_calls") {
                                return Ok(StreamChunk::Done);
                            }
                        }

                        // Empty chunk for incomplete data
                        Ok(StreamChunk::Content(String::new()))
                    } else {
                        Ok(StreamChunk::Done)
                    }
                }
                Err(e) => Err(anyhow::anyhow!("Stream error: {}", e)),
            }
        });

        Ok(Box::pin(mapped_stream))
    }
}
