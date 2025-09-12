use color_eyre::Result;

use crate::llm::provider::{Context, ModelProvider};
use crate::types::LlmResponse;

pub struct FakeLlmProvider {
    responses: Vec<Vec<LlmResponse>>,
    call_count: std::sync::atomic::AtomicUsize,
}

impl FakeLlmProvider {
    pub fn new(responses: Vec<Vec<LlmResponse>>) -> Self {
        Self {
            responses,
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn with_single_response(chunks: Vec<LlmResponse>) -> Self {
        Self::new(vec![chunks])
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl ModelProvider for FakeLlmProvider {
    fn generate_response(
        &self,
        _request: Context,
    ) -> impl tokio_stream::Stream<Item = Result<LlmResponse>> + Send {
        let current_call = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let response = if current_call < self.responses.len() {
            self.responses[current_call].clone()
        } else if !self.responses.is_empty() {
            // Repeat the last response if we run out
            self.responses.last().unwrap().clone()
        } else {
            vec![LlmResponse::Done]
        };

        tokio_stream::iter(response.into_iter().map(Ok))
    }
}
