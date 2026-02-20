use std::sync::{Arc, Mutex};

use crate::{Context, LlmResponse, LlmResponseStream, StreamingModelProvider};

pub struct FakeLlmProvider {
    responses: Vec<Vec<LlmResponse>>,
    call_count: std::sync::atomic::AtomicUsize,
    /// Captured contexts from each call to `stream_response`
    captured_contexts: Arc<Mutex<Vec<Context>>>,
}

impl FakeLlmProvider {
    pub fn new(responses: Vec<Vec<LlmResponse>>) -> Self {
        Self {
            responses,
            call_count: std::sync::atomic::AtomicUsize::new(0),
            captured_contexts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_single_response(chunks: Vec<LlmResponse>) -> Self {
        Self::new(vec![chunks])
    }

    /// Returns a handle to the captured contexts that can be used to verify
    /// what contexts were passed to the LLM.
    pub fn captured_contexts(&self) -> Arc<Mutex<Vec<Context>>> {
        Arc::clone(&self.captured_contexts)
    }
}

impl StreamingModelProvider for FakeLlmProvider {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        // Capture the context for later verification
        if let Ok(mut contexts) = self.captured_contexts.lock() {
            contexts.push(context.clone());
        }

        let current_call = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let response = if current_call < self.responses.len() {
            self.responses[current_call].clone()
        } else {
            vec![LlmResponse::done()]
        };

        Box::pin(tokio_stream::iter(response.into_iter().map(Ok)))
    }

    fn display_name(&self) -> String {
        "Fake LLM".to_string()
    }
}
