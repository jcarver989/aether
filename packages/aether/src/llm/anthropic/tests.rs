#[cfg(test)]
mod integration_tests {
    use super::super::provider::AnthropicProvider;
    use super::super::types::{Role, SystemContent};
    use crate::llm::provider::Context;
    use crate::types::{ChatMessage, IsoString, LlmResponse, ToolCallRequest, ToolDefinition};
    use futures::StreamExt;
    use serde_json::json;
    use tokio_stream;

    fn create_test_provider() -> AnthropicProvider {
        AnthropicProvider::new("test-key".to_string())
            .unwrap()
            .with_model("claude-3-5-sonnet-20241022".to_string())
            .with_base_url("http://localhost:8080".to_string())
            .with_temperature(0.7)
            .with_max_tokens(1000)
            .with_prompt_caching(false)
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let provider = AnthropicProvider::new("test-key".to_string());
        assert!(provider.is_ok(), "Provider creation should succeed");
    }

    // Note: This test would require a mock HTTP server to avoid making actual API calls
    // For now, we rely on unit tests for the core functionality

    #[tokio::test]
    async fn test_system_message_and_tools() {
        let provider = create_test_provider();

        let context = Context {
            messages: vec![
                ChatMessage::System {
                    content: "You are a helpful assistant.".to_string(),
                    timestamp: IsoString::now(),
                },
                ChatMessage::User {
                    content: "What's the weather like?".to_string(),
                    timestamp: IsoString::now(),
                },
            ],
            tools: vec![ToolDefinition {
                name: "get_weather".to_string(),
                description: "Get current weather information".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city and state"
                        }
                    },
                    "required": ["location"]
                })
                .to_string(),
                server: None,
            }],
        };

        let request = provider.build_request(context).unwrap();

        if let Some(system) = &request.system {
            match system {
                SystemContent::Text(text) => {
                    assert_eq!(text, "You are a helpful assistant.");
                }
                _ => panic!("Expected text system content"),
            }
        } else {
            panic!("Expected system prompt");
        }
        assert_eq!(request.messages.len(), 1);
        assert!(request.tools.is_some());
        assert_eq!(request.tools.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_assistant_message_with_tool_calls() {
        let provider = create_test_provider();

        let context = Context {
            messages: vec![
                ChatMessage::User {
                    content: "What's the weather like?".to_string(),
                    timestamp: IsoString::now(),
                },
                ChatMessage::Assistant {
                    content: "I'll check the weather for you.".to_string(),
                    timestamp: IsoString::now(),
                    tool_calls: vec![ToolCallRequest {
                        id: "call_123".to_string(),
                        name: "get_weather".to_string(),
                        arguments: r#"{"location": "San Francisco, CA"}"#.to_string(),
                    }],
                },
                ChatMessage::ToolCallResult {
                    tool_call_id: "call_123".to_string(),
                    content: "Sunny, 72°F".to_string(),
                    timestamp: IsoString::now(),
                },
            ],
            tools: vec![],
        };

        let request = provider.build_request(context).unwrap();
        assert_eq!(request.messages.len(), 3);

        let assistant_msg = &request.messages[1];
        assert_eq!(assistant_msg.role, Role::Assistant);

        let tool_result_msg = &request.messages[2];
        assert_eq!(tool_result_msg.role, Role::User);
    }

    #[tokio::test]
    async fn test_prompt_caching_enabled() {
        let provider = AnthropicProvider::new("test-key".to_string()).unwrap(); // Caching enabled by default

        let context = Context {
            messages: vec![ChatMessage::User {
                content: "Hello with caching".to_string(),
                timestamp: IsoString::now(),
            }],
            tools: vec![ToolDefinition {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                parameters: r#"{"type": "object", "properties": {}}"#.to_string(),
                server: None,
            }],
        };

        let request = provider.build_request(context).unwrap();

        // With caching enabled by default, but no system message, no caching should occur
        if let Some(tools) = request.tools {
            // Tools don't have cache_control - they're auto-cached when system prompt is cached
            assert!(tools[0].cache_control.is_none());
        }

        // User messages don't get cache_control in the new hierarchy
        let user_msg = &request.messages[0];
        assert!(user_msg.cache_control.is_none());
    }

    #[tokio::test]
    async fn test_error_handling_invalid_json_tool_params() {
        let provider = create_test_provider();

        let context = Context {
            messages: vec![ChatMessage::User {
                content: "Test".to_string(),
                timestamp: IsoString::now(),
            }],
            tools: vec![ToolDefinition {
                name: "invalid_tool".to_string(),
                description: "Tool with invalid JSON".to_string(),
                parameters: "invalid json".to_string(),
                server: None,
            }],
        };

        let result = provider.build_request(context);
        assert!(result.is_err(), "Should fail with invalid JSON parameters");
    }

    #[tokio::test]
    async fn test_empty_context() {
        let provider = create_test_provider();

        let context = Context {
            messages: vec![],
            tools: vec![],
        };

        let request = provider.build_request(context).unwrap();
        assert!(request.messages.is_empty());
        assert!(request.tools.is_none());
        assert!(request.system.is_none());
    }

    #[tokio::test]
    async fn test_temperature_and_max_tokens() {
        let provider = AnthropicProvider::new("test-key".to_string())
            .unwrap()
            .with_temperature(0.9)
            .with_max_tokens(2048);

        let context = Context {
            messages: vec![ChatMessage::User {
                content: "Test".to_string(),
                timestamp: IsoString::now(),
            }],
            tools: vec![],
        };

        let request = provider.build_request(context).unwrap();
        assert_eq!(request.temperature, Some(0.9));
        assert_eq!(request.max_tokens, 2048);
    }

    #[tokio::test]
    async fn test_streaming_response_error_handling() {
        use super::super::streaming::process_anthropic_stream;

        let error_lines = vec!["data: {\"type\": \"error\", \"error\": {\"type\": \"invalid_request_error\", \"message\": \"Invalid API key\"}}".to_string()];

        let stream = tokio_stream::iter(error_lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_anthropic_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result);
        }

        assert!(responses.len() >= 2);
        assert!(matches!(responses[0], Ok(LlmResponse::Start { .. })));

        let has_error = responses.iter().any(|r| r.is_err());
        assert!(has_error, "Should contain an error response");
    }
}
