mod utils;

use crate::utils::*;
use aether_core::llm::provider::StreamChunkStream;
use aether_core::llm::{ChatMessage, ChatRequest, LlmProvider, StreamChunk};
use async_trait::async_trait;
use color_eyre::Result;
use tokio_stream::StreamExt;

struct MockErrorProvider {
    error_after: usize,
}

impl MockErrorProvider {
    fn new(error_after: usize) -> Self {
        Self { error_after }
    }
}

#[async_trait]
impl LlmProvider for MockErrorProvider {
    async fn complete_stream_chunks(&self, _request: ChatRequest) -> Result<StreamChunkStream> {
        let error_after = self.error_after;
        let stream = tokio_stream::iter((0..error_after + 1).map(move |i| {
            if i >= error_after {
                Err(color_eyre::eyre::eyre!("Network error"))
            } else if i == 0 {
                Ok(StreamChunk::Content { content: "Hello".to_string() })
            } else {
                Ok(StreamChunk::Content { content: " chunk".to_string() })
            }
        }));
        Ok(Box::pin(stream))
    }
}

#[tokio::test]
async fn test_basic_content_streaming() -> Result<()> {
    let provider = FakeLlmProvider::with_content_chunks(vec!["Hello", " ", "world"]);
    let request = create_test_chat_request(vec![ChatMessage::User {
        content: "test".to_string(),
    }]);

    let stream = provider.complete_stream_chunks(request).await?;
    let content = collect_stream_content(stream).await?;

    assert_eq!(content, "Hello world");
    Ok(())
}

#[tokio::test]
async fn test_tool_call_streaming() -> Result<()> {
    let provider = FakeLlmProvider::with_tool_call(
        "Let me use a tool.",
        TEST_TOOL_ID,
        "test_tool",
        r#"{"param": "value"}"#,
    );
    let request = create_test_chat_request(vec![ChatMessage::User {
        content: "test".to_string(),
    }]);

    let mut stream = provider.complete_stream_chunks(request).await?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        match chunk {
            StreamChunk::Content { content: text } => content.push_str(&text),
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
            StreamChunk::ToolCallComplete { .. } => {
                // Tool call is complete
            }
            StreamChunk::Done => break,
        }
    }

    assert_eq!(content, "Let me use a tool.");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].0, TEST_TOOL_ID);
    assert_eq!(tool_calls[0].1, "test_tool");
    assert_eq!(tool_calls[0].2, r#"{"param": "value"}"#);

    Ok(())
}

#[tokio::test]
async fn test_stream_error_handling() -> Result<()> {
    let provider = MockErrorProvider::new(1);
    let request = create_test_chat_request(vec![ChatMessage::User {
        content: "test".to_string(),
    }]);

    let mut stream = provider.complete_stream_chunks(request).await?;
    let mut content = String::new();
    let mut error_encountered = false;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(StreamChunk::Content { content: text }) => content.push_str(&text),
            Ok(StreamChunk::Done) => break,
            Err(_) => {
                error_encountered = true;
                break;
            }
            _ => {}
        }
    }

    assert_eq!(content, "Hello");
    assert!(error_encountered);
    Ok(())
}

#[tokio::test]
async fn test_empty_stream() -> Result<()> {
    let provider = FakeLlmProvider::with_content("");
    let request = create_test_chat_request(vec![ChatMessage::User {
        content: "test".to_string(),
    }]);

    let stream = provider.complete_stream_chunks(request).await?;
    let content = collect_stream_content(stream).await?;

    assert_eq!(content, "");
    Ok(())
}

#[tokio::test]
async fn test_multiple_tool_calls_streaming() -> Result<()> {
    let chunks = vec![
        StreamChunk::Content { content: "I'll call multiple tools.".to_string() },
        StreamChunk::ToolCallStart {
            id: "call_1".to_string(),
            name: "tool_a".to_string(),
        },
        StreamChunk::ToolCallArgument {
            id: "call_1".to_string(),
            argument: r#"{"arg": "1"}"#.to_string(),
        },
        StreamChunk::ToolCallComplete {
            id: "call_1".to_string(),
        },
        StreamChunk::ToolCallStart {
            id: "call_2".to_string(),
            name: "tool_b".to_string(),
        },
        StreamChunk::ToolCallArgument {
            id: "call_2".to_string(),
            argument: r#"{"arg": "2"}"#.to_string(),
        },
        StreamChunk::ToolCallComplete {
            id: "call_2".to_string(),
        },
        StreamChunk::Done,
    ];

    let provider = FakeLlmProvider::new(chunks);
    let request = create_test_chat_request(vec![ChatMessage::User {
        content: "test".to_string(),
    }]);

    let mut stream = provider.complete_stream_chunks(request).await?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        match chunk {
            StreamChunk::Content { content: text } => content.push_str(&text),
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
            StreamChunk::Done => break,
        }
    }

    assert_eq!(content, "I'll call multiple tools.");
    assert_eq!(tool_calls.len(), 2);

    assert_eq!(tool_calls[0].0, "call_1");
    assert_eq!(tool_calls[0].1, "tool_a");
    assert_eq!(tool_calls[0].2, r#"{"arg": "1"}"#);

    assert_eq!(tool_calls[1].0, "call_2");
    assert_eq!(tool_calls[1].1, "tool_b");
    assert_eq!(tool_calls[1].2, r#"{"arg": "2"}"#);

    Ok(())
}

#[tokio::test]
async fn test_streaming_chunk_serialization() -> Result<()> {
    let chunks = vec![
        StreamChunk::Content { content: "test".to_string() },
        StreamChunk::ToolCallStart {
            id: TEST_TOOL_ID.to_string(),
            name: "test_tool".to_string(),
        },
        StreamChunk::ToolCallArgument {
            id: TEST_TOOL_ID.to_string(),
            argument: "{}".to_string(),
        },
        StreamChunk::ToolCallComplete {
            id: TEST_TOOL_ID.to_string(),
        },
        StreamChunk::Done,
    ];

    for chunk in chunks {
        let serialized = serde_json::to_string(&chunk)?;
        let deserialized: StreamChunk = serde_json::from_str(&serialized)?;

        // Verify the chunk round-trips correctly using helper
        assert_stream_chunk_matches(&chunk, &deserialized);
    }

    Ok(())
}
