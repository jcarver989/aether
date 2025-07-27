use aether::llm::provider::StreamChunkStream;
use aether::llm::{ChatMessage, ChatRequest, LlmProvider, StreamChunk};
use anyhow::Result;
use async_trait::async_trait;
use tokio_stream::StreamExt;

struct MockStreamingProvider {
    chunks: Vec<StreamChunk>,
}

impl MockStreamingProvider {
    fn new(chunks: Vec<StreamChunk>) -> Self {
        Self { chunks }
    }
}

struct MockErrorProvider {
    error_after: usize,
}

impl MockErrorProvider {
    fn new(error_after: usize) -> Self {
        Self { error_after }
    }
}

#[async_trait]
impl LlmProvider for MockStreamingProvider {
    async fn complete_stream_chunks(&self, _request: ChatRequest) -> Result<StreamChunkStream> {
        let chunks = self.chunks.clone();
        let stream = tokio_stream::iter(chunks.into_iter().map(|c| Ok(c)));
        Ok(Box::pin(stream))
    }
}

#[async_trait]
impl LlmProvider for MockErrorProvider {
    async fn complete_stream_chunks(&self, _request: ChatRequest) -> Result<StreamChunkStream> {
        let error_after = self.error_after;
        let stream = tokio_stream::iter((0..error_after + 1).map(move |i| {
            if i >= error_after {
                Err(anyhow::anyhow!("Network error"))
            } else if i == 0 {
                Ok(StreamChunk::Content("Hello".to_string()))
            } else {
                Ok(StreamChunk::Content(" chunk".to_string()))
            }
        }));
        Ok(Box::pin(stream))
    }
}

#[tokio::test]
async fn test_basic_content_streaming() -> Result<()> {
    let chunks = vec![
        StreamChunk::Content("Hello".to_string()),
        StreamChunk::Content(" ".to_string()),
        StreamChunk::Content("world".to_string()),
        StreamChunk::Done,
    ];

    let provider = MockStreamingProvider::new(chunks);
    let request = ChatRequest {
        messages: vec![ChatMessage::User {
            content: "test".to_string(),
        }],
        tools: vec![],
        temperature: None,
    };

    let mut stream = provider.complete_stream_chunks(request).await?;
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

    assert_eq!(content, "Hello world");
    assert!(done);
    Ok(())
}

#[tokio::test]
async fn test_tool_call_streaming() -> Result<()> {
    let chunks = vec![
        StreamChunk::Content("Let me use a tool.".to_string()),
        StreamChunk::ToolCallStart {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
        },
        StreamChunk::ToolCallArgument {
            id: "call_123".to_string(),
            argument: r#"{"param": "value"}"#.to_string(),
        },
        StreamChunk::ToolCallComplete {
            id: "call_123".to_string(),
        },
        StreamChunk::Done,
    ];

    let provider = MockStreamingProvider::new(chunks);
    let request = ChatRequest {
        messages: vec![ChatMessage::User {
            content: "test".to_string(),
        }],
        tools: vec![],
        temperature: None,
    };

    let mut stream = provider.complete_stream_chunks(request).await?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();

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
            StreamChunk::ToolCallComplete { .. } => {
                // Tool call is complete
            }
            StreamChunk::Done => break,
        }
    }

    assert_eq!(content, "Let me use a tool.");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].0, "call_123");
    assert_eq!(tool_calls[0].1, "test_tool");
    assert_eq!(tool_calls[0].2, r#"{"param": "value"}"#);

    Ok(())
}

#[tokio::test]
async fn test_stream_error_handling() -> Result<()> {
    let provider = MockErrorProvider::new(1);
    let request = ChatRequest {
        messages: vec![ChatMessage::User {
            content: "test".to_string(),
        }],
        tools: vec![],
        temperature: None,
    };

    let mut stream = provider.complete_stream_chunks(request).await?;
    let mut content = String::new();
    let mut error_encountered = false;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(StreamChunk::Content(text)) => content.push_str(&text),
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
    let chunks = vec![StreamChunk::Done];

    let provider = MockStreamingProvider::new(chunks);
    let request = ChatRequest {
        messages: vec![ChatMessage::User {
            content: "test".to_string(),
        }],
        tools: vec![],
        temperature: None,
    };

    let mut stream = provider.complete_stream_chunks(request).await?;
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

    assert_eq!(content, "");
    assert!(done);
    Ok(())
}

#[tokio::test]
async fn test_multiple_tool_calls_streaming() -> Result<()> {
    let chunks = vec![
        StreamChunk::Content("I'll call multiple tools.".to_string()),
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

    let provider = MockStreamingProvider::new(chunks);
    let request = ChatRequest {
        messages: vec![ChatMessage::User {
            content: "test".to_string(),
        }],
        tools: vec![],
        temperature: None,
    };

    let mut stream = provider.complete_stream_chunks(request).await?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();

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
        StreamChunk::Content("test".to_string()),
        StreamChunk::ToolCallStart {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
        },
        StreamChunk::ToolCallArgument {
            id: "call_123".to_string(),
            argument: "{}".to_string(),
        },
        StreamChunk::ToolCallComplete {
            id: "call_123".to_string(),
        },
        StreamChunk::Done,
    ];

    for chunk in chunks {
        let serialized = serde_json::to_string(&chunk)?;
        let deserialized: StreamChunk = serde_json::from_str(&serialized)?;

        // Verify the chunk round-trips correctly
        match (&chunk, &deserialized) {
            (StreamChunk::Content(a), StreamChunk::Content(b)) => assert_eq!(a, b),
            (
                StreamChunk::ToolCallStart {
                    id: id1,
                    name: name1,
                },
                StreamChunk::ToolCallStart {
                    id: id2,
                    name: name2,
                },
            ) => {
                assert_eq!(id1, id2);
                assert_eq!(name1, name2);
            }
            (
                StreamChunk::ToolCallArgument {
                    id: id1,
                    argument: arg1,
                },
                StreamChunk::ToolCallArgument {
                    id: id2,
                    argument: arg2,
                },
            ) => {
                assert_eq!(id1, id2);
                assert_eq!(arg1, arg2);
            }
            (
                StreamChunk::ToolCallComplete { id: id1 },
                StreamChunk::ToolCallComplete { id: id2 },
            ) => {
                assert_eq!(id1, id2);
            }
            (StreamChunk::Done, StreamChunk::Done) => {}
            _ => panic!("Chunk serialization mismatch"),
        }
    }

    Ok(())
}
