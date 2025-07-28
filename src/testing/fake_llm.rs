use anyhow::Result;
use async_trait::async_trait;
use tokio_stream::StreamExt;

use crate::llm::provider::{ChatRequest, LlmProvider, StreamChunk, StreamChunkStream};

pub struct FakeLlmProvider {
    responses: Vec<Vec<StreamChunk>>,
    call_count: std::sync::atomic::AtomicUsize,
}

impl FakeLlmProvider {
    pub fn new(responses: Vec<Vec<StreamChunk>>) -> Self {
        Self {
            responses,
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn with_single_response(chunks: Vec<StreamChunk>) -> Self {
        Self::new(vec![chunks])
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmProvider for FakeLlmProvider {
    async fn complete_stream_chunks(&self, _request: ChatRequest) -> Result<StreamChunkStream> {
        let current_call = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let response = if current_call < self.responses.len() {
            self.responses[current_call].clone()
        } else if !self.responses.is_empty() {
            // Repeat the last response if we run out
            self.responses.last().unwrap().clone()
        } else {
            vec![StreamChunk::Done]
        };

        let stream = tokio_stream::iter(response.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider::ChatMessage;

    #[tokio::test]
    async fn test_fake_llm_returns_canned_responses() {
        let responses = vec![
            vec![
                StreamChunk::Content("Hello".to_string()),
                StreamChunk::Content(" world!".to_string()),
                StreamChunk::Done,
            ],
            vec![
                StreamChunk::Content("Second response".to_string()),
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
        assert_eq!(chunks[0], StreamChunk::Content("Hello".to_string()));
        assert_eq!(chunks[1], StreamChunk::Content(" world!".to_string()));
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
            StreamChunk::Content("Second response".to_string())
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
            StreamChunk::Content("Second response".to_string())
        );
        assert_eq!(chunks[1], StreamChunk::Done);

        assert_eq!(fake_llm.call_count(), 3);
    }

    #[tokio::test]
    async fn test_fake_llm_with_tool_calls() {
        let responses = vec![vec![
            StreamChunk::Content("Let me help you with that.".to_string()),
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
}
