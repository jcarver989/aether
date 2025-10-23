use aether::llm::openai_compatible::ChatCompletionStreamResponse;
use serde_json;

/// Test that we can deserialize the actual Z.ai response format
/// The error showed this JSON structure:
/// {"id":"202510181239034578873d49744455","created":1760762344,"model":"glm-4.6","choices":[{"index":0,"delta":{"role":"assistant","content":"\n"}}]}

#[test]
fn test_deserialize_zai_response_missing_object_field() {
    // This is the actual format Z.ai returns - missing the "object" field
    let json = r#"{
        "id": "202510181239034578873d49744455",
        "created": 1760762344,
        "model": "glm-4.6",
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": "\n"
            }
        }]
    }"#;

    // This should not panic and should deserialize successfully
    let result: Result<ChatCompletionStreamResponse, _> = serde_json::from_str(json);

    assert!(
        result.is_ok(),
        "Failed to deserialize Z.ai response: {:?}",
        result.err()
    );

    let response = result.unwrap();
    assert_eq!(response.id, "202510181239034578873d49744455");
    assert_eq!(response.model, "glm-4.6");
    assert_eq!(response.created, 1760762344);
    // Should have default object value
    assert_eq!(response.object, "chat.completion.chunk");
    assert_eq!(response.choices.len(), 1);

    let choice = &response.choices[0];
    assert_eq!(choice.index, 0);
    assert_eq!(
        choice.delta.role,
        Some(async_openai::types::Role::Assistant)
    );
    assert_eq!(choice.delta.content, Some("\n".to_string()));
}

#[test]
fn test_deserialize_zai_response_with_finish_reason() {
    let json = r#"{
        "id": "test123",
        "created": 1760762344,
        "model": "glm-4.6",
        "choices": [{
            "index": 0,
            "delta": {
                "content": "Hello world"
            },
            "finish_reason": "stop"
        }]
    }"#;

    let result: Result<ChatCompletionStreamResponse, _> = serde_json::from_str(json);

    assert!(result.is_ok());
    let response = result.unwrap();

    assert_eq!(
        response.choices[0].delta.content,
        Some("Hello world".to_string())
    );
    assert!(response.choices[0].finish_reason.is_some());
}

#[test]
fn test_convert_to_openai_type() {
    let json = r#"{
        "id": "test123",
        "created": 1760762344,
        "model": "glm-4.6",
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": "test"
            }
        }]
    }"#;

    let response: ChatCompletionStreamResponse = serde_json::from_str(json).unwrap();

    // Convert to standard OpenAI type
    let openai_response: async_openai::types::CreateChatCompletionStreamResponse = response.into();

    assert_eq!(openai_response.id, "test123");
    assert_eq!(openai_response.model, "glm-4.6");
    assert_eq!(openai_response.created, 1760762344);
    assert_eq!(openai_response.object, "chat.completion.chunk");
}

#[test]
fn test_deserialize_zai_response_with_tool_calls() {
    let json = r#"{
        "id": "test123",
        "created": 1760762344,
        "model": "glm-4.6",
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "tool_calls": [{
                    "index": 0,
                    "id": "call_abc123",
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "arguments": "{\"location\":"
                    }
                }]
            }
        }]
    }"#;

    let result: Result<ChatCompletionStreamResponse, _> = serde_json::from_str(json);

    assert!(result.is_ok(), "Failed to deserialize: {:?}", result.err());
    let response = result.unwrap();

    let choice = &response.choices[0];
    assert!(choice.delta.tool_calls.is_some());

    let tool_calls = choice.delta.tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls.len(), 1);

    let tool_call = &tool_calls[0];
    assert_eq!(tool_call.index, 0);
    assert_eq!(tool_call.id, Some("call_abc123".to_string()));
    assert_eq!(tool_call.tool_type, Some("function".to_string()));

    let function = tool_call.function.as_ref().unwrap();
    assert_eq!(function.name, Some("get_weather".to_string()));
    assert_eq!(function.arguments, Some("{\"location\":".to_string()));
}
