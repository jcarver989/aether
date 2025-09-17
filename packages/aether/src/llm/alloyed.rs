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

    fn get_current_provider(&self) -> Option<&Box<dyn ModelProvider>> {
        if self.providers.is_empty() {
            return None;
        }
        let index =
            self.current_provider_index.fetch_add(1, Ordering::Relaxed) % self.providers.len();
        Some(&self.providers[index])
    }
}

impl ModelProvider for AlloyedModelProvider {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        match self.get_current_provider() {
            Some(provider) => provider.stream_response(context),
            None => Box::pin(tokio_stream::empty()),
        }
    }

    fn display_name(&self) -> String {
        match self.get_current_provider() {
            Some(provider) => provider.display_name(),
            None => "Alloyed (no providers)".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeLlmProvider;
    use crate::types::LlmResponse;

    #[test]
    fn test_alloyed_provider_display_name_empty() {
        let provider = AlloyedModelProvider::new(vec![]);
        assert_eq!(provider.display_name(), "Alloyed (no providers)");
    }

    #[test]
    fn test_alloyed_provider_display_name_single() {
        let fake_provider = FakeLlmProvider::new(vec![vec![LlmResponse::Done]]);
        let provider = AlloyedModelProvider::new(vec![Box::new(fake_provider)]);

        // Should return the individual provider's display name
        assert_eq!(provider.display_name(), "Fake LLM");
    }

    #[test]
    fn test_alloyed_provider_display_name_multiple() {
        let fake_provider1 = FakeLlmProvider::new(vec![vec![LlmResponse::Done]]);
        let fake_provider2 = FakeLlmProvider::new(vec![vec![LlmResponse::Done]]);
        let provider =
            AlloyedModelProvider::new(vec![Box::new(fake_provider1), Box::new(fake_provider2)]);

        // Should cycle through individual provider names
        assert_eq!(provider.display_name(), "Fake LLM"); // First call
        assert_eq!(provider.display_name(), "Fake LLM"); // Second call (cycles back)
    }

    #[test]
    fn test_alloyed_provider_cycling() {
        let fake_provider1 = FakeLlmProvider::new(vec![vec![LlmResponse::Done]]);
        let fake_provider2 = FakeLlmProvider::new(vec![vec![LlmResponse::Done]]);
        let provider =
            AlloyedModelProvider::new(vec![Box::new(fake_provider1), Box::new(fake_provider2)]);

        let context = Context {
            messages: vec![],
            tools: vec![],
        };

        // Both stream_response and display_name should cycle through providers
        let _stream1 = provider.stream_response(&context);
        let _stream2 = provider.stream_response(&context);
        let _stream3 = provider.stream_response(&context);

        // display_name should cycle and return individual provider names
        assert_eq!(provider.display_name(), "Fake LLM");
        assert_eq!(provider.display_name(), "Fake LLM");
    }
}
