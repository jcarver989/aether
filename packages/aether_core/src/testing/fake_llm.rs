use color_eyre::Result;

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
