use aether::llm::openai_compatible::types::{
    ChatCompletionStreamChoice, ChatCompletionStreamResponse, ChatCompletionStreamResponseDelta,
    Usage,
};
use aether::llm::openai::streaming::process_completion_stream;
use aether::llm::LlmResponse;
use async_openai::types::chat::Role;
use tokio_stream::StreamExt;

/// Test that OpenRouter usage data is correctly extracted from stream responses
#[tokio::test]
async fn test_openrouter_usage_extraction() {
    // Create a stream response that includes usage data, similar to what OpenRouter returns
    let stream_items = vec![
        // First chunk with content
        Ok::<ChatCompletionStreamResponse, std::io::Error>(ChatCompletionStreamResponse {
            id: "gen-123".to_string(),
            choices: vec![ChatCompletionStreamChoice {
                index: 0,
                delta: ChatCompletionStreamResponseDelta {
                    role: Some(Role::Assistant),
                    content: Some("Hello".to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            created: 1234567890,
            model: "openai/gpt-3.5-turbo".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: None,
        }),
        // Second chunk with more content
        Ok::<ChatCompletionStreamResponse, std::io::Error>(ChatCompletionStreamResponse {
            id: "gen-123".to_string(),
            choices: vec![ChatCompletionStreamChoice {
                index: 0,
                delta: ChatCompletionStreamResponseDelta {
                    role: None,
                    content: Some(" world".to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            created: 1234567890,
            model: "openai/gpt-3.5-turbo".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: None,
        }),
        // Final chunk with usage data
        Ok::<ChatCompletionStreamResponse, std::io::Error>(ChatCompletionStreamResponse {
            id: "gen-123".to_string(),
            choices: vec![ChatCompletionStreamChoice {
                index: 0,
                delta: ChatCompletionStreamResponseDelta {
                    role: None,
                    content: None,
                    tool_calls: None,
                },
                finish_reason: Some(aether::llm::openai_compatible::types::FinishReason::Stop),
                logprobs: None,
            }],
            created: 1234567890,
            model: "openai/gpt-3.5-turbo".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            }),
        }),
    ];

    // Convert to standard OpenAI format and process
    let stream = tokio_stream::iter(
        stream_items
            .into_iter()
            .map(|r| r.map(|response| response.into())),
    );
    let mut processed_stream = Box::pin(process_completion_stream(stream));

    let mut events = Vec::new();
    while let Some(event) = processed_stream.next().await {
        events.push(event.unwrap());
    }

    // Verify we got usage data
    let usage_events: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            LlmResponse::Usage {
                input_tokens,
                output_tokens,
            } => Some((input_tokens, output_tokens)),
            _ => None,
        })
        .collect();

    assert_eq!(usage_events.len(), 1, "Should have exactly one usage event");
    assert_eq!(*usage_events[0].0, 10, "Input tokens should be 10");
    assert_eq!(*usage_events[0].1, 20, "Output tokens should be 20");
}

/// Test that negative token counts are handled correctly
#[tokio::test]
async fn test_openrouter_negative_token_handling() {
    // OpenRouter sometimes returns negative token counts
    // Our conversion should handle this by clamping to 0
    let stream_items = vec![Ok::<ChatCompletionStreamResponse, std::io::Error>(
        ChatCompletionStreamResponse {
            id: "gen-123".to_string(),
            choices: vec![ChatCompletionStreamChoice {
                index: 0,
                delta: ChatCompletionStreamResponseDelta {
                    role: None,
                    content: None,
                    tool_calls: None,
                },
                finish_reason: Some(aether::llm::openai_compatible::types::FinishReason::Stop),
                logprobs: None,
            }],
            created: 1234567890,
            model: "openai/gpt-3.5-turbo".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: Some(Usage {
                prompt_tokens: -5, // Negative value
                completion_tokens: 10,
                total_tokens: 5,
            }),
        },
    )];

    let stream = tokio_stream::iter(
        stream_items
            .into_iter()
            .map(|r| r.map(|response| response.into())),
    );
    let mut processed_stream = Box::pin(process_completion_stream(stream));

    let mut events = Vec::new();
    while let Some(event) = processed_stream.next().await {
        events.push(event.unwrap());
    }

    // Verify negative tokens are clamped to 0
    let usage_events: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            LlmResponse::Usage {
                input_tokens,
                output_tokens,
            } => Some((input_tokens, output_tokens)),
            _ => None,
        })
        .collect();

    assert_eq!(usage_events.len(), 1);
    assert_eq!(
        *usage_events[0].0, 0,
        "Negative input tokens should be clamped to 0"
    );
    assert_eq!(*usage_events[0].1, 10, "Output tokens should be 10");
}

/// Test that the OpenRouterChatRequest serializes the usage parameter correctly
#[test]
fn test_openrouter_request_serialization() {
    use aether::llm::openrouter::{OpenRouterChatRequest, OpenRouterUsage};
    use async_openai::types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent,
    };
    use serde_json;

    let request = OpenRouterChatRequest {
        model: "openai/gpt-3.5-turbo".to_string(),
        messages: vec![ChatCompletionRequestMessage::User(
            ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text("Hello".to_string()),
                name: None,
            },
        )],
        stream: Some(true),
        tools: None,
        tool_choice: None,
        temperature: None,
        top_p: None,
        max_completion_tokens: None,
        stream_options: None,
        usage: Some(OpenRouterUsage { include: true }),
        presence_penalty: None,
        frequency_penalty: None,
        stop: None,
        response_format: None,
    };

    let json = serde_json::to_value(&request).unwrap();

    // Verify the usage parameter is serialized correctly
    assert_eq!(
        json["usage"]["include"],
        serde_json::Value::Bool(true),
        "Usage parameter should be serialized with include: true"
    );
    assert_eq!(
        json["model"],
        serde_json::Value::String("openai/gpt-3.5-turbo".to_string()),
        "Model should be serialized correctly"
    );
}
