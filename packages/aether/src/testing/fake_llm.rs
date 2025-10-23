use crate::llm::{Context, LlmResponse, LlmResponseStream, StreamingModelProvider};

pub struct FakeLlmProvider {
    name: String,
    responses: Vec<Vec<LlmResponse>>,
    call_count: std::sync::atomic::AtomicUsize,
}

impl FakeLlmProvider {
    pub fn new(responses: Vec<Vec<LlmResponse>>) -> Self {
        Self {
            name: "Fake LLM".to_string(),
            responses,
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn with_single_response(chunks: Vec<LlmResponse>) -> Self {
        Self::new(vec![chunks])
    }

    pub fn with_name(name: &str, responses: Vec<Vec<LlmResponse>>) -> Self {
        Self {
            name: name.to_string(),
            responses,
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl StreamingModelProvider for FakeLlmProvider {
    fn stream_response(&self, _context: &Context) -> LlmResponseStream {
        let current_call = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let response = if current_call < self.responses.len() {
            self.responses[current_call].clone()
        } else {
            vec![LlmResponse::Done]
        };

        Box::pin(tokio_stream::iter(response.into_iter().map(Ok)))
    }

    fn display_name(&self) -> String {
        self.name.clone()
    }
}
