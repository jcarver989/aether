use aether::llm::{
    LlmProvider, ChatRequest, ChatMessage, ChatResponse, ToolDefinition, ToolCall,
    ProviderConfig, create_provider, StreamChunk, StreamChunkStream, ChatStream
};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use tokio_stream::iter;

struct FakeOpenRouterProvider {
    model: String,
}

impl FakeOpenRouterProvider {
    fn new(model: String) -> Self {
        Self { model }
    }
}

#[async_trait]
impl LlmProvider for FakeOpenRouterProvider {
    async fn complete(&self, _request: ChatRequest) -> Result<ChatResponse> {
        Ok(ChatResponse {
            content: "Hello from fake OpenRouter!".to_string(),
            tool_calls: vec![],
        })
    }

    async fn complete_stream(&self, _request: ChatRequest) -> Result<ChatStream> {
        let items = vec![
            Ok("Hello".to_string()),
            Ok(" from".to_string()),
            Ok(" fake".to_string()),
            Ok(" OpenRouter!".to_string()),
        ];
        
        let stream = iter(items);
        Ok(Box::pin(stream))
    }

    async fn complete_stream_chunks(&self, _request: ChatRequest) -> Result<StreamChunkStream> {
        let chunks = vec![
            Ok(StreamChunk::Content("Hello".to_string())),
            Ok(StreamChunk::Content(" from".to_string())),
            Ok(StreamChunk::Content(" fake".to_string())),
            Ok(StreamChunk::Content(" OpenRouter!".to_string())),
            Ok(StreamChunk::Done),
        ];
        
        let stream = iter(chunks);
        Ok(Box::pin(stream))
    }

    fn get_model(&self) -> &str {
        &self.model
    }
}

struct FakeOllamaProvider {
    model: String,
}

impl FakeOllamaProvider {
    fn new(model: String) -> Self {
        Self { model }
    }
}

#[async_trait]
impl LlmProvider for FakeOllamaProvider {
    async fn complete(&self, _request: ChatRequest) -> Result<ChatResponse> {
        Ok(ChatResponse {
            content: "Hello from fake Ollama!".to_string(),
            tool_calls: vec![],
        })
    }

    async fn complete_stream(&self, _request: ChatRequest) -> Result<ChatStream> {
        let items = vec![
            Ok("Hello".to_string()),
            Ok(" from".to_string()),
            Ok(" fake".to_string()),
            Ok(" Ollama!".to_string()),
        ];
        
        let stream = iter(items);
        Ok(Box::pin(stream))
    }

    async fn complete_stream_chunks(&self, _request: ChatRequest) -> Result<StreamChunkStream> {
        let chunks = vec![
            Ok(StreamChunk::Content("Hello".to_string())),
            Ok(StreamChunk::Content(" from".to_string())),
            Ok(StreamChunk::Content(" fake".to_string())),
            Ok(StreamChunk::Content(" Ollama!".to_string())),
            Ok(StreamChunk::Done),
        ];
        
        let stream = iter(chunks);
        Ok(Box::pin(stream))
    }

    fn get_model(&self) -> &str {
        &self.model
    }
}

#[tokio::test]
async fn test_provider_trait_complete() -> Result<()> {
    let fake_provider = FakeOpenRouterProvider::new("test-model".to_string());
    
    let request = ChatRequest {
        messages: vec![ChatMessage::User { 
            content: "Hello, world!".to_string() 
        }],
        tools: vec![],
        temperature: Some(0.7),
    };
    
    let response = fake_provider.complete(request).await?;
    assert_eq!(response.content, "Hello from fake OpenRouter!");
    assert!(response.tool_calls.is_empty());
    assert_eq!(fake_provider.get_model(), "test-model");
    
    Ok(())
}

#[tokio::test]
async fn test_provider_trait_complete_stream() -> Result<()> {
    use tokio_stream::StreamExt;
    
    let fake_provider = FakeOllamaProvider::new("test-model".to_string());
    
    let request = ChatRequest {
        messages: vec![ChatMessage::User { 
            content: "Hello, world!".to_string() 
        }],
        tools: vec![],
        temperature: Some(0.7),
    };
    
    let mut stream = fake_provider.complete_stream(request).await?;
    let mut collected = String::new();
    
    while let Some(chunk) = stream.next().await {
        collected.push_str(&chunk?);
    }
    
    assert_eq!(collected, "Hello from fake Ollama!");
    
    Ok(())
}

#[tokio::test]
async fn test_provider_trait_complete_stream_chunks() -> Result<()> {
    use tokio_stream::StreamExt;
    
    let fake_provider = FakeOpenRouterProvider::new("test-model".to_string());
    
    let request = ChatRequest {
        messages: vec![ChatMessage::User { 
            content: "Hello, world!".to_string() 
        }],
        tools: vec![],
        temperature: Some(0.7),
    };
    
    let mut stream = fake_provider.complete_stream_chunks(request).await?;
    let mut content = String::new();
    let mut done = false;
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        match chunk {
            StreamChunk::Content(text) => content.push_str(&text),
            StreamChunk::Done => {
                done = true;
                break;
            }
            _ => {}
        }
    }
    
    assert_eq!(content, "Hello from fake OpenRouter!");
    assert!(done);
    
    Ok(())
}

#[tokio::test]
async fn test_chat_request_with_tools() -> Result<()> {
    let fake_provider = FakeOpenRouterProvider::new("test-model".to_string());
    
    let tool = ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get the current weather".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city name"
                }
            },
            "required": ["location"]
        }),
    };
    
    let request = ChatRequest {
        messages: vec![
            ChatMessage::System { 
                content: "You are a helpful assistant.".to_string() 
            },
            ChatMessage::User { 
                content: "What's the weather like?".to_string() 
            }
        ],
        tools: vec![tool],
        temperature: Some(0.5),
    };
    
    let response = fake_provider.complete(request).await?;
    assert_eq!(response.content, "Hello from fake OpenRouter!");
    
    Ok(())
}

#[tokio::test]
async fn test_chat_messages_variants() -> Result<()> {
    let fake_provider = FakeOllamaProvider::new("test-model".to_string());
    
    let request = ChatRequest {
        messages: vec![
            ChatMessage::System { 
                content: "System message".to_string() 
            },
            ChatMessage::User { 
                content: "User message".to_string() 
            },
            ChatMessage::Assistant { 
                content: "Assistant message".to_string() 
            },
            ChatMessage::Tool { 
                tool_call_id: "call_123".to_string(),
                content: "Tool result".to_string() 
            },
        ],
        tools: vec![],
        temperature: None,
    };
    
    let response = fake_provider.complete(request).await?;
    assert_eq!(response.content, "Hello from fake Ollama!");
    
    Ok(())
}

#[test]
fn test_provider_config_variants() {
    let openrouter_config = ProviderConfig::OpenRouter {
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
    };
    
    let ollama_config = ProviderConfig::Ollama {
        base_url: Some("http://localhost:11434".to_string()),
        model: "llama2".to_string(),
    };
    
    match openrouter_config {
        ProviderConfig::OpenRouter { api_key, model } => {
            assert_eq!(api_key, "test-key");
            assert_eq!(model, "test-model");
        },
        _ => panic!("Expected OpenRouter config"),
    }
    
    match ollama_config {
        ProviderConfig::Ollama { base_url, model } => {
            assert_eq!(base_url, Some("http://localhost:11434".to_string()));
            assert_eq!(model, "llama2");
        },
        _ => panic!("Expected Ollama config"),
    }
}

#[test]
fn test_provider_serialization() -> Result<()> {
    let tool = ToolDefinition {
        name: "test_tool".to_string(),
        description: "A test tool".to_string(),
        parameters: json!({"type": "object"}),
    };
    
    let message = ChatMessage::User {
        content: "Test message".to_string(),
    };
    
    let request = ChatRequest {
        messages: vec![message],
        tools: vec![tool],
        temperature: Some(0.8),
    };
    
    // Test that the types can be serialized and deserialized
    let serialized = serde_json::to_string(&request)?;
    let deserialized: ChatRequest = serde_json::from_str(&serialized)?;
    
    assert_eq!(deserialized.temperature, Some(0.8));
    assert_eq!(deserialized.messages.len(), 1);
    assert_eq!(deserialized.tools.len(), 1);
    
    Ok(())
}

#[tokio::test]
async fn test_tool_call_response() -> Result<()> {
    struct ToolCallProvider;
    
    #[async_trait]
    impl LlmProvider for ToolCallProvider {
        async fn complete(&self, _request: ChatRequest) -> Result<ChatResponse> {
            Ok(ChatResponse {
                content: "I'll call a tool for you.".to_string(),
                tool_calls: vec![
                    ToolCall {
                        id: "call_123".to_string(),
                        name: "get_weather".to_string(),
                        arguments: json!({"location": "San Francisco"}),
                    }
                ],
            })
        }

        async fn complete_stream(&self, _request: ChatRequest) -> Result<ChatStream> {
            let stream = iter(vec![Ok("Test".to_string())]);
            Ok(Box::pin(stream))
        }

        async fn complete_stream_chunks(&self, _request: ChatRequest) -> Result<StreamChunkStream> {
            let stream = iter(vec![Ok(StreamChunk::Done)]);
            Ok(Box::pin(stream))
        }

        fn get_model(&self) -> &str {
            "test-model"
        }
    }
    
    let provider = ToolCallProvider;
    let request = ChatRequest {
        messages: vec![ChatMessage::User { 
            content: "What's the weather?".to_string() 
        }],
        tools: vec![],
        temperature: None,
    };
    
    let response = provider.complete(request).await?;
    assert_eq!(response.content, "I'll call a tool for you.");
    assert_eq!(response.tool_calls.len(), 1);
    
    let tool_call = &response.tool_calls[0];
    assert_eq!(tool_call.id, "call_123");
    assert_eq!(tool_call.name, "get_weather");
    assert_eq!(tool_call.arguments, json!({"location": "San Francisco"}));
    
    Ok(())
}

#[test]
fn test_tool_call_serialization() -> Result<()> {
    let tool_call = ToolCall {
        id: "call_456".to_string(),
        name: "read_file".to_string(),
        arguments: json!({"path": "/etc/hosts"}),
    };
    
    let serialized = serde_json::to_string(&tool_call)?;
    let deserialized: ToolCall = serde_json::from_str(&serialized)?;
    
    assert_eq!(deserialized.id, "call_456");
    assert_eq!(deserialized.name, "read_file");
    assert_eq!(deserialized.arguments, json!({"path": "/etc/hosts"}));
    
    Ok(())
}