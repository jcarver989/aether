use color_eyre::Result;

use crate::llm::provider::{ChatRequest, LlmProvider};
use crate::types::LlmMessage;

pub struct FakeLlmProvider {
    responses: Vec<Vec<LlmMessage>>,
    call_count: std::sync::atomic::AtomicUsize,
}

impl FakeLlmProvider {
    pub fn new(responses: Vec<Vec<LlmMessage>>) -> Self {
        Self {
            responses,
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn with_single_response(chunks: Vec<LlmMessage>) -> Self {
        Self::new(vec![chunks])
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl LlmProvider for FakeLlmProvider {
    fn complete_stream_chunks(
        &self,
        _request: ChatRequest,
    ) -> impl tokio_stream::Stream<Item = Result<LlmMessage>> + Send {
        let current_call = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let response = if current_call < self.responses.len() {
            self.responses[current_call].clone()
        } else if !self.responses.is_empty() {
            // Repeat the last response if we run out
            self.responses.last().unwrap().clone()
        } else {
            vec![LlmMessage::Done]
        };

        tokio_stream::iter(response.into_iter().map(Ok))
    }
}
