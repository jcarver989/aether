use crate::llm::provider::{Context, LlmResponseStream, StreamingModelProvider};
use crate::types::LlmResponse;

pub fn fake_llm(messages: &[Vec<LlmResponse>]) -> FakeLlmProvider {
    FakeLlmProvider::new(Vec::from(messages))
}

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

impl StreamingModelProvider for FakeLlmProvider {
    fn stream_response(&self, _context: &Context) -> LlmResponseStream {
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

        Box::pin(tokio_stream::iter(response.into_iter().map(Ok)))
    }

    fn display_name(&self) -> String {
        "Fake LLM".to_string()
    }
}
