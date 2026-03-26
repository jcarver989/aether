use async_openai::types::chat::Role;
use llm::LlmResponse;
use llm::providers::openai::streaming::process_completion_stream;
use llm::providers::openai_compatible;
use llm::providers::openai_compatible::types::{
    ChatCompletionStreamChoice, ChatCompletionStreamResponse, ChatCompletionStreamResponseDelta,
    Usage,
};
use tokio_stream::StreamExt;

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
                    reasoning_content: None,
                    tool_calls: None,
                },
                finish_reason: Some(openai_compatible::types::FinishReason::Stop),
                logprobs: None,
            }],
            created: 1_234_567_890,
            model: "openai/gpt-3.5-turbo".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: Some(Usage {
                prompt_tokens: -5, // Negative value
                completion_tokens: 10,
                total_tokens: 5,
                prompt_tokens_details: None,
            }),
        },
    )];

    let stream = tokio_stream::iter(
        stream_items
            .into_iter()
            .map(|r| r.map(std::convert::Into::into)),
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
                ..
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

/// Test that usage data is captured when sent in a separate chunk after `finish_reason`
/// This matches `OpenRouter`'s actual behavior where usage comes in the "last SSE message"
/// See: <https://openrouter.ai/docs/guides/usage-accounting>
#[tokio::test]
async fn test_openrouter_usage_in_separate_final_chunk() {
    let stream_items: Vec<Result<ChatCompletionStreamResponse, std::io::Error>> = vec![
        // Chunk 1: Content
        Ok(ChatCompletionStreamResponse {
            id: "gen-123".to_string(),
            choices: vec![ChatCompletionStreamChoice {
                index: 0,
                delta: ChatCompletionStreamResponseDelta {
                    role: Some(Role::Assistant),
                    content: Some("Hello world".to_string()),
                    reasoning_content: None,
                    tool_calls: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            created: 1_234_567_890,
            model: "openai/gpt-3.5-turbo".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: None,
        }),
        // Chunk 2: finish_reason but NO usage yet
        Ok(ChatCompletionStreamResponse {
            id: "gen-123".to_string(),
            choices: vec![ChatCompletionStreamChoice {
                index: 0,
                delta: ChatCompletionStreamResponseDelta {
                    role: None,
                    content: None,
                    reasoning_content: None,
                    tool_calls: None,
                },
                finish_reason: Some(openai_compatible::types::FinishReason::Stop),
                logprobs: None,
            }],
            created: 1_234_567_890,
            model: "openai/gpt-3.5-turbo".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: None, // No usage in this chunk!
        }),
        // Chunk 3: Usage data in separate final chunk with empty choices
        Ok(ChatCompletionStreamResponse {
            id: "gen-123".to_string(),
            choices: vec![], // Empty choices array
            created: 1_234_567_890,
            model: "openai/gpt-3.5-turbo".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: Some(Usage {
                prompt_tokens: 15,
                completion_tokens: 25,
                total_tokens: 40,
                prompt_tokens_details: None,
            }),
        }),
    ];

    // Convert to standard OpenAI format and process
    let stream = tokio_stream::iter(
        stream_items
            .into_iter()
            .map(|r| r.map(std::convert::Into::into)),
    );
    let mut processed_stream = Box::pin(process_completion_stream(stream));

    let mut events = Vec::new();
    while let Some(event) = processed_stream.next().await {
        events.push(event.unwrap());
    }

    // Verify we got usage data from the separate final chunk
    let usage_events: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            LlmResponse::Usage {
                input_tokens,
                output_tokens,
                ..
            } => Some((*input_tokens, *output_tokens)),
            _ => None,
        })
        .collect();

    assert_eq!(
        usage_events.len(),
        1,
        "Should have exactly one usage event even when usage is in separate chunk after finish_reason"
    );
    assert_eq!(usage_events[0].0, 15, "Input tokens should be 15");
    assert_eq!(usage_events[0].1, 25, "Output tokens should be 25");

    // Also verify we got the text content
    let text_events: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            LlmResponse::Text { chunk } => Some(chunk.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(text_events.len(), 1);
    assert_eq!(text_events[0], "Hello world");
}

/// Test that the `OpenRouterChatRequest` serializes the usage parameter correctly
#[test]
fn test_openrouter_request_serialization() {
    use llm::providers::openai_compatible::types::CompatibleChatMessage;
    use llm::providers::openrouter::{CacheControl, OpenRouterChatRequest, OpenRouterUsage};
    use serde_json;

    let request = OpenRouterChatRequest {
        model: "openai/gpt-3.5-turbo".to_string(),
        messages: vec![CompatibleChatMessage::User {
            content: llm::providers::openai_compatible::types::UserContent::Text(
                "Hello".to_string(),
            ),
        }],
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
        reasoning_effort: None,
        cache_control: Some(CacheControl::ephemeral()),
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

/// Test that cache_control serializes as `{"type": "ephemeral"}` at the request root
#[test]
fn test_openrouter_cache_control_serialization() {
    use llm::providers::openrouter::CacheControl;

    let cache_control = CacheControl::ephemeral();
    let json = serde_json::to_value(&cache_control).unwrap();

    assert_eq!(json["type"], "ephemeral");
}

/// Test that `From<CompatibleChatRequest>` sets cache_control for prompt caching
#[test]
fn test_openrouter_from_compatible_request_includes_cache_control() {
    use llm::providers::openai_compatible::CompatibleChatRequest;
    use llm::providers::openai_compatible::types::CompatibleChatMessage;
    use llm::providers::openrouter::OpenRouterChatRequest;

    let compatible = CompatibleChatRequest {
        model: "zhipu/glm-5".to_string(),
        messages: vec![
            CompatibleChatMessage::System {
                content: "You are helpful.".to_string(),
            },
            CompatibleChatMessage::User {
                content: llm::providers::openai_compatible::types::UserContent::Text(
                    "Hello".to_string(),
                ),
            },
        ],
        stream: Some(true),
        tools: None,
        stream_options: None,
        reasoning_effort: None,
    };

    let openrouter: OpenRouterChatRequest = compatible.into();

    let json = serde_json::to_value(&openrouter).unwrap();
    assert_eq!(
        json["cache_control"]["type"], "ephemeral",
        "From conversion should set cache_control for prompt caching"
    );
}
