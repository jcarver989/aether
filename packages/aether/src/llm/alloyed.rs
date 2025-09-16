use crate::llm::{
    ModelProvider,
    provider::{Context, LlmResponseStream},
};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A ModelProvider that alternates models on every turn via a round-robin strategy.
/// Alternating between models with different strengths and weaknesses can improve the agent's performance.
pub struct AlloyedModelProvider {
    providers: Vec<Box<dyn ModelProvider>>,
    current_provider_index: AtomicUsize,
}

impl AlloyedModelProvider {
    pub fn new(providers: Vec<Box<dyn ModelProvider>>) -> Self {
        Self {
            providers,
            current_provider_index: AtomicUsize::new(0),
        }
    }
}

impl ModelProvider for AlloyedModelProvider {
    fn stream_response(&self, context: Context) -> LlmResponseStream {
        if self.providers.is_empty() {
            return Box::pin(tokio_stream::empty());
        }

        let index =
            self.current_provider_index.fetch_add(1, Ordering::Relaxed) % self.providers.len();
        let provider = &self.providers[index];
        provider.stream_response(context)
    }
}
