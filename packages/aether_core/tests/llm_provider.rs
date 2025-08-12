mod utils;

use crate::utils::*;
use aether_core::llm::openrouter_types::CustomChatCompletionStreamResponse;
use aether_core::llm::provider::ToolCall;
use aether_core::llm::{ChatMessage, ChatRequest, LlmProvider, StreamChunk};
use color_eyre::Result;
use serde_json::json;
use tokio;

#[tokio::test]
async fn test_openrouter_provider_stream_chunks() -> Result<()> {
    let fake_provider =
        FakeLlmProvider::with_content_chunks(vec!["Hello", " from", " fake", " OpenRouter!"]);

    let request = create_test_chat_request(vec![ChatMessage::User {
        content: "Hello, world!".to_string(),
    }]);

    let stream = fake_provider.complete_stream_chunks(request).await?;
    let content = collect_stream_content(stream).await?;

    assert_eq!(content, "Hello from fake OpenRouter!");
    Ok(())
}

#[tokio::test]
async fn test_ollama_provider_stream_chunks() -> Result<()> {
    let fake_provider =
        FakeLlmProvider::with_content_chunks(vec!["Hello", " from", " fake", " Ollama!"]);

    let request = create_test_chat_request(vec![ChatMessage::User {
        content: "Hello, world!".to_string(),
    }]);

    let stream = fake_provider.complete_stream_chunks(request).await?;
    let content = collect_stream_content(stream).await?;

    assert_eq!(content, "Hello from fake Ollama!");
    Ok(())
}

#[tokio::test]
async fn test_stream_chunks_with_tools() -> Result<()> {
    let fake_provider =
        FakeLlmProvider::with_content_chunks(vec!["Hello", " from", " fake", " OpenRouter!"]);

    let tool = create_test_tool_definition("get_weather", "Get the current weather");

    let request = create_test_chat_request_with_tools(
        vec![
            ChatMessage::System {
                content: "You are a helpful assistant.".to_string(),
            },
            ChatMessage::User {
                content: "What's the weather like?".to_string(),
            },
        ],
        vec![tool],
    );

    let stream = fake_provider.complete_stream_chunks(request).await?;
    let content = collect_stream_content(stream).await?;

    assert_eq!(content, "Hello from fake OpenRouter!");
    Ok(())
}

#[tokio::test]
async fn test_chat_messages_variants() -> Result<()> {
    let fake_provider =
        FakeLlmProvider::with_content_chunks(vec!["Hello", " from", " fake", " Ollama!"]);

    let request = create_test_chat_request(vec![
        ChatMessage::System {
            content: "System message".to_string(),
        },
        ChatMessage::User {
            content: "User message".to_string(),
        },
        ChatMessage::Assistant {
            content: "Assistant message".to_string(),
            tool_calls: None,
        },
        ChatMessage::Tool {
            tool_call_id: TEST_TOOL_ID.to_string(),
            content: "Tool result".to_string(),
        },
    ]);

    let stream = fake_provider.complete_stream_chunks(request).await?;
    let content = collect_stream_content(stream).await?;

    assert_eq!(content, "Hello from fake Ollama!");
    Ok(())
}

#[test]
fn test_chat_request_serialization() -> Result<()> {
    let tool = create_test_tool_definition("test_tool", "A test tool");

    let message = ChatMessage::User {
        content: "Test message".to_string(),
    };

    let request = create_test_chat_request_with_tools(vec![message], vec![tool]);

    // Test that the types can be serialized and deserialized
    let serialized = serde_json::to_string(&request)?;
    let deserialized: ChatRequest = serde_json::from_str(&serialized)?;

    assert_eq!(deserialized.temperature, Some(0.7));
    assert_eq!(deserialized.messages.len(), 1);
    assert_eq!(deserialized.tools.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_tool_call_stream_chunks() -> Result<()> {
    use tokio_stream::StreamExt;

    let provider = FakeLlmProvider::with_tool_call(
        "I'll call a tool: ",
        TEST_TOOL_ID,
        "get_weather",
        r#"{"location": "San Francisco"}"#,
    );

    let request = create_test_chat_request(vec![ChatMessage::User {
        content: "What's the weather?".to_string(),
    }]);

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
                if let Some((_, _, args)) =
                    tool_calls.iter_mut().find(|(call_id, _, _)| call_id == &id)
                {
                    args.push_str(&argument);
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
    assert_eq!(tool_calls[0].0, TEST_TOOL_ID);
    assert_eq!(tool_calls[0].1, "get_weather");
    assert_eq!(tool_calls[0].2, r#"{"location": "San Francisco"}"#);
    assert!(done);

    Ok(())
}

#[test]
fn test_tool_call_serialization() -> Result<()> {
    let tool_call = create_test_tool_call("call_456", "read_file", json!({"path": "/etc/hosts"}));

    let serialized = serde_json::to_string(&tool_call)?;
    let deserialized: ToolCall = serde_json::from_str(&serialized)?;

    assert_eq!(deserialized.id, "call_456");
    assert_eq!(deserialized.name, "read_file");
    assert_eq!(deserialized.arguments, json!({"path": "/etc/hosts"}));

    Ok(())
}

#[test]
fn test_negative_index_deserialization() -> Result<()> {
    // Test JSON response with negative index value like the one that was failing
    let json_response = r#"{
        "id": "gen-1753765148-50rFLkRXr4ZevuYmygAk",
        "provider": "Novita",
        "model": "qwen/qwen3-coder",
        "object": "chat.completion.chunk",
        "created": 1753765148,
        "choices": [{
            "index": -1,
            "delta": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "index": -1,
                    "id": "call_057df76479b648488008ffb7",
                    "type": "function",
                    "function": {
                        "name": "invoke-tool",
                        "arguments": "{\"args\": \"<parameter=path>\\n/projects/aether\"}"
                    }
                }]
            },
            "finish_reason": null,
            "native_finish_reason": null,
            "logprobs": null
        }],
        "system_fingerprint": ""
    }"#;

    // This should not panic or fail - it should successfully deserialize with negative index
    let response: CustomChatCompletionStreamResponse = serde_json::from_str(json_response)?;

    assert_eq!(response.id, "gen-1753765148-50rFLkRXr4ZevuYmygAk");
    assert_eq!(response.choices.len(), 1);
    assert_eq!(response.choices[0].index, -1); // Verify negative index is preserved

    Ok(())
}
