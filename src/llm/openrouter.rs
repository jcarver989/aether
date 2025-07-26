use anyhow::Result;
use async_openai::{
    Client, 
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, 
        CreateChatCompletionRequest, 
        FunctionObject,
        ChatCompletionTool,
        ChatCompletionToolType,
        ChatCompletionRequestToolMessage,
        ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage,
        ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestAssistantMessageContent,
        ChatCompletionRequestToolMessageContent,
    }
};
use async_trait::async_trait;
use tokio_stream::StreamExt;

use super::provider::{LlmProvider, ChatRequest, ChatMessage, ChatResponse, ToolCall, ToolDefinition, ChatStream, StreamChunk, StreamChunkStream};

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
        messages.into_iter().map(|msg| match msg {
            ChatMessage::System { content } => {
                ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                    content: content.into(),
                    name: None,
                })
            },
            ChatMessage::User { content } => {
                ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                    content: content.into(),
                    name: None,
                })
            },
            ChatMessage::Assistant { content } => {
                ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                    content: Some(ChatCompletionRequestAssistantMessageContent::Text(content)),
                    name: None,
                    tool_calls: None,
                    function_call: None,
                    audio: None,
                    refusal: None,
                })
            },
            ChatMessage::Tool { tool_call_id, content } => {
                ChatCompletionRequestMessage::Tool(ChatCompletionRequestToolMessage {
                    content: ChatCompletionRequestToolMessageContent::Text(content),
                    tool_call_id,
                })
            },
        }).collect()
    }
    
    fn convert_tools(&self, tools: Vec<ToolDefinition>) -> Vec<ChatCompletionTool> {
        tools.into_iter().map(|tool| {
            ChatCompletionTool {
                r#type: ChatCompletionToolType::Function,
                function: FunctionObject {
                    name: tool.name,
                    description: Some(tool.description),
                    parameters: Some(tool.parameters),
                    strict: Some(false),
                },
            }
        }).collect()
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    async fn complete(&self, request: ChatRequest) -> Result<ChatResponse> {
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
            stream: Some(false),
            ..Default::default()
        };
        
        let response = self.client.chat().create(req).await?;
        
        let choice = response.choices.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No response choices returned"))?;
        
        let content = choice.message.content.unwrap_or_default();
        let tool_calls = choice.message.tool_calls.unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.function.name,
                arguments: serde_json::from_str(&tc.function.arguments).unwrap_or_default(),
            })
            .collect();
        
        Ok(ChatResponse { content, tool_calls })
    }
    
    async fn complete_stream(&self, request: ChatRequest) -> Result<ChatStream> {
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
                        if let Some(content) = &choice.delta.content {
                            Ok(content.clone())
                        } else {
                            Ok(String::new())
                        }
                    } else {
                        Ok(String::new())
                    }
                },
                Err(e) => Err(anyhow::anyhow!("Stream error: {}", e)),
            }
        });
        
        Ok(Box::pin(mapped_stream))
    }

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
                                // Log to debug file
                                if let Ok(mut debug_file) = std::fs::OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open("/tmp/aether_debug.log") {
                                    use std::io::Write;
                                    let _ = writeln!(debug_file, "[{}] Stream: Got finish_reason: {:?}, current_tool_id: {:?}", 
                                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), finish_reason, current_tool_id);
                                }
                                
                                // Complete any pending tool call
                                if format!("{:?}", finish_reason).contains("tool_calls") {
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
    
    fn get_model(&self) -> &str {
        &self.model
    }
}