use aether_core::llm::provider::{ChatMessage, ChatRequest, LlmProvider};
use aether_core::testing::FakeLlmProvider;
use aether_core::types::StreamEvent;
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_fake_llm_returns_canned_responses() {
    let responses = vec![
        vec![
            StreamEvent::Content {
                chunk: "Hello".to_string(),
            },
            StreamEvent::Content {
                chunk: " world!".to_string(),
            },
            StreamEvent::Done,
        ],
        vec![
            StreamEvent::Content {
                chunk: "Second response".to_string(),
            },
            StreamEvent::Done,
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
    assert_eq!(
        chunks[0],
        StreamEvent::Content {
            chunk: "Hello".to_string()
        }
    );
    assert_eq!(
        chunks[1],
        StreamEvent::Content {
            chunk: " world!".to_string()
        }
    );
    assert_eq!(chunks[2], StreamEvent::Done);

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
        StreamEvent::Content {
            chunk: "Second response".to_string()
        }
    );
    assert_eq!(chunks[1], StreamEvent::Done);

    // Third call (should repeat last response)
    let mut stream = fake_llm.complete_stream_chunks(request).await.unwrap();
    let mut chunks = vec![];
    while let Some(chunk) = stream.next().await {
        chunks.push(chunk.unwrap());
    }

    assert_eq!(chunks.len(), 2);
    assert_eq!(
        chunks[0],
        StreamEvent::Content {
            chunk: "Second response".to_string()
        }
    );
    assert_eq!(chunks[1], StreamEvent::Done);

    assert_eq!(fake_llm.call_count(), 3);
}

#[tokio::test]
async fn test_fake_llm_with_tool_calls() {
    let responses = [vec![
        StreamEvent::Content {
            chunk: "Let me help you with that.".to_string(),
        },
        StreamEvent::ToolCallStart {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
        },
        StreamEvent::ToolCallArgument {
            id: "call_123".to_string(),
            chunk: r#"{"location": "San Francisco"}"#.to_string(),
        },
        StreamEvent::ToolCallComplete {
            id: "call_123".to_string(),
        },
        StreamEvent::Done,
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
    assert!(matches!(chunks[1], StreamEvent::ToolCallStart { .. }));
    assert!(matches!(chunks[2], StreamEvent::ToolCallArgument { .. }));
    assert!(matches!(chunks[3], StreamEvent::ToolCallComplete { .. }));
}
