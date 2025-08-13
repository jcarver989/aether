use aether_core::testing::FakeLlmProvider;
use aether_core::llm::provider::{ChatRequest, ChatMessage, StreamChunk, LlmProvider};
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_fake_llm_returns_canned_responses() {
    let responses = vec![
        vec![
            StreamChunk::Content { content: "Hello".to_string() },
            StreamChunk::Content { content: " world!".to_string() },
            StreamChunk::Done,
        ],
        vec![
            StreamChunk::Content { content: "Second response".to_string() },
            StreamChunk::Done,
        ],
    ];

    let fake_llm = FakeLlmProvider::new(responses);

    // First call
    let request = ChatRequest {
        messages: vec![ChatMessage::User {
            content: "Test".to_string(),
        }],
        tools: vec![],
        temperature: None,
    };

    let mut stream = fake_llm
        .complete_stream_chunks(request.clone())
        .await
        .unwrap();
    let mut chunks = vec![];
    while let Some(chunk) = stream.next().await {
        chunks.push(chunk.unwrap());
    }

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0], StreamChunk::Content { content: "Hello".to_string() });
    assert_eq!(chunks[1], StreamChunk::Content { content: " world!".to_string() });
    assert_eq!(chunks[2], StreamChunk::Done);

    // Second call
    let mut stream = fake_llm
        .complete_stream_chunks(request.clone())
        .await
        .unwrap();
    let mut chunks = vec![];
    while let Some(chunk) = stream.next().await {
        chunks.push(chunk.unwrap());
    }

    assert_eq!(chunks.len(), 2);
    assert_eq!(
        chunks[0],
        StreamChunk::Content { content: "Second response".to_string() }
    );
    assert_eq!(chunks[1], StreamChunk::Done);

    // Third call (should repeat last response)
    let mut stream = fake_llm.complete_stream_chunks(request).await.unwrap();
    let mut chunks = vec![];
    while let Some(chunk) = stream.next().await {
        chunks.push(chunk.unwrap());
    }

    assert_eq!(chunks.len(), 2);
    assert_eq!(
        chunks[0],
        StreamChunk::Content { content: "Second response".to_string() }
    );
    assert_eq!(chunks[1], StreamChunk::Done);

    assert_eq!(fake_llm.call_count(), 3);
}

#[tokio::test]
async fn test_fake_llm_with_tool_calls() {
    let responses = [vec![
        StreamChunk::Content { content: "Let me help you with that.".to_string() },
        StreamChunk::ToolCallStart {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
        },
        StreamChunk::ToolCallArgument {
            id: "call_123".to_string(),
            argument: r#"{"location": "San Francisco"}"#.to_string(),
        },
        StreamChunk::ToolCallComplete {
            id: "call_123".to_string(),
        },
        StreamChunk::Done,
    ]];

    let fake_llm = FakeLlmProvider::with_single_response(responses[0].clone());

    let request = ChatRequest {
        messages: vec![ChatMessage::User {
            content: "What's the weather?".to_string(),
        }],
        tools: vec![],
        temperature: None,
    };

    let mut stream = fake_llm.complete_stream_chunks(request).await.unwrap();
    let mut chunks = vec![];
    while let Some(chunk) = stream.next().await {
        chunks.push(chunk.unwrap());
    }

    assert_eq!(chunks.len(), 5);
    assert!(matches!(chunks[1], StreamChunk::ToolCallStart { .. }));
    assert!(matches!(chunks[2], StreamChunk::ToolCallArgument { .. }));
    assert!(matches!(chunks[3], StreamChunk::ToolCallComplete { .. }));
}