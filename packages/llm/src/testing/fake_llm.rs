use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::{Context, LlmError, LlmResponse, LlmResponseStream, StreamingModelProvider};

pub struct FakeLlmProvider {
    responses: Vec<Vec<Result<LlmResponse, LlmError>>>,
    call_count: AtomicUsize,
    /// Captured contexts from each call to `stream_response`
    captured_contexts: Arc<Mutex<Vec<Context>>>,
    display_name: String,
    context_window: Option<u32>,
}

impl FakeLlmProvider {
    pub fn new(responses: Vec<Vec<LlmResponse>>) -> Self {
        let wrapped = responses.into_iter().map(|turn| turn.into_iter().map(Ok).collect()).collect();
        Self::from_results(wrapped)
    }

    pub fn with_single_response(chunks: Vec<LlmResponse>) -> Self {
        Self::new(vec![chunks])
    }

    pub fn from_results(responses: Vec<Vec<Result<LlmResponse, LlmError>>>) -> Self {
        Self {
            responses,
            call_count: AtomicUsize::new(0),
            captured_contexts: Arc::new(Mutex::new(Vec::new())),
            display_name: "Fake LLM".to_string(),
            context_window: None,
        }
    }

    pub fn with_display_name(mut self, name: &str) -> Self {
        self.display_name = name.to_string();
        self
    }

    pub fn with_context_window(mut self, window: Option<u32>) -> Self {
        self.context_window = window;
        self
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

        let current_call = self.call_count.fetch_add(1, Ordering::SeqCst);

        let response = if current_call < self.responses.len() {
            self.responses[current_call].clone()
        } else {
            vec![Ok(LlmResponse::done())]
        };

        Box::pin(tokio_stream::iter(response))
    }

    fn display_name(&self) -> String {
        self.display_name.clone()
    }

    fn context_window(&self) -> Option<u32> {
        self.context_window
    }
}
