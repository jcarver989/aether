use aether::llm::{
    LlmProvider, ChatRequest, ChatMessage, ToolDefinition, StreamChunk
};
use aether::llm::provider::{ToolCall, StreamChunkStream};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use tokio_stream::iter;

struct FakeOpenRouterProvider {
    _model: String,
}

impl FakeOpenRouterProvider {
    fn new(model: String) -> Self {
        Self { _model: model }
    }
}

#[async_trait]
impl LlmProvider for FakeOpenRouterProvider {
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
}

struct FakeOllamaProvider {
    _model: String,
}

impl FakeOllamaProvider {
    fn new(model: String) -> Self {
        Self { _model: model }
    }
}

#[async_trait]
impl LlmProvider for FakeOllamaProvider {
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
}

#[tokio::test]
async fn test_openrouter_provider_stream_chunks() -> Result<()> {
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
async fn test_ollama_provider_stream_chunks() -> Result<()> {
    use tokio_stream::StreamExt;
    
    let fake_provider = FakeOllamaProvider::new("test-model".to_string());
    
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
    
    assert_eq!(content, "Hello from fake Ollama!");
    assert!(done);
    
    Ok(())
}

#[tokio::test]
async fn test_stream_chunks_with_tools() -> Result<()> {
    use tokio_stream::StreamExt;
    
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
async fn test_chat_messages_variants() -> Result<()> {
    use tokio_stream::StreamExt;
    
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
    
    assert_eq!(content, "Hello from fake Ollama!");
    assert!(done);
    
    Ok(())
}

#[test]
fn test_chat_request_serialization() -> Result<()> {
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
async fn test_tool_call_stream_chunks() -> Result<()> {
    use tokio_stream::StreamExt;
    
    struct ToolCallProvider;
    
    #[async_trait]
    impl LlmProvider for ToolCallProvider {
        async fn complete_stream_chunks(&self, _request: ChatRequest) -> Result<StreamChunkStream> {
            let chunks = vec![
                Ok(StreamChunk::Content("I'll call a tool: ".to_string())),
                Ok(StreamChunk::ToolCallStart { 
                    id: "call_123".to_string(), 
                    name: "get_weather".to_string() 
                }),
                Ok(StreamChunk::ToolCallArgument { 
                    id: "call_123".to_string(), 
                    argument: r#"{"location": "San Francisco"}"#.to_string() 
                }),
                Ok(StreamChunk::ToolCallComplete { id: "call_123".to_string() }),
                Ok(StreamChunk::Done),
            ];
            
            let stream = iter(chunks);
            Ok(Box::pin(stream))
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
    
    let mut stream = provider.complete_stream_chunks(request).await?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    let mut done = false;
    
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        match chunk {
            StreamChunk::Content(text) => content.push_str(&text),
            StreamChunk::ToolCallStart { id, name } => {
                tool_calls.push((id, name, String::new()));
            }
            StreamChunk::ToolCallArgument { id, argument } => {
                if let Some(call) = tool_calls.iter_mut().find(|(call_id, _, _)| call_id == &id) {
                    call.2.push_str(&argument);
                }
            }
            StreamChunk::ToolCallComplete { .. } => {}
            StreamChunk::Done => {
                done = true;
                break;
            }
        }
    }
    
    assert_eq!(content, "I'll call a tool: ");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].0, "call_123");
    assert_eq!(tool_calls[0].1, "get_weather");
    assert_eq!(tool_calls[0].2, r#"{"location": "San Francisco"}"#);
    assert!(done);
    
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