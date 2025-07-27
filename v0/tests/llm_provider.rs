use aether::llm::{
    LlmProvider, ChatRequest, ChatMessage, ChatResponse, ToolDefinition,
    ProviderConfig, create_provider
};
use anyhow::Result;
use serde_json::json;

struct FakeOpenRouterProvider {
    model: String,
}

impl FakeOpenRouterProvider {
    fn new(model: String) -> Self {
        Self { model }
    }
}

#[async_trait::async_trait]
impl LlmProvider for FakeOpenRouterProvider {
    async fn complete(&self, _request: ChatRequest) -> Result<ChatResponse> {
        Ok(ChatResponse {
            content: "Hello from fake OpenRouter!".to_string(),
            tool_calls: vec![],
        })
    }

    async fn complete_stream(&self, _request: ChatRequest) -> Result<aether::llm::ChatStream> {
        use tokio_stream::{Stream, iter};
        use std::pin::Pin;
        
        let items = vec![
            Ok("Hello".to_string()),
            Ok(" from".to_string()),
            Ok(" fake".to_string()),
            Ok(" OpenRouter!".to_string()),
        ];
        
        let stream = iter(items);
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

#[async_trait::async_trait]
impl LlmProvider for FakeOllamaProvider {
    async fn complete(&self, _request: ChatRequest) -> Result<ChatResponse> {
        Ok(ChatResponse {
            content: "Hello from fake Ollama!".to_string(),
            tool_calls: vec![],
        })
    }

    async fn complete_stream(&self, _request: ChatRequest) -> Result<aether::llm::ChatStream> {
        use tokio_stream::{Stream, iter};
        use std::pin::Pin;
        
        let items = vec![
            Ok("Hello".to_string()),
            Ok(" from".to_string()),
            Ok(" fake".to_string()),
            Ok(" Ollama!".to_string()),
        ];
        
        let stream = iter(items);
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