use crate::LlmModel;
use crate::Result as LlmResult;
use std::pin::Pin;
use tokio_stream::Stream;

use super::{Context, LlmResponse};

// We use Box<dyn> here instead of impl Stream primarily to support a nicer user-facing API for
// alloyed models -- i.e. it allows us to have Vec<Box<dyn ModelProvider>> in AlloyedModelProvider
pub type LlmResponseStream = Pin<Box<dyn Stream<Item = LlmResult<LlmResponse>> + Send>>;

/// Factory trait for constructing model providers
///
/// This trait is separate from `StreamingModelProvider` to allow trait objects
/// (Box<dyn StreamingModelProvider>) to work without construction methods.
pub trait ProviderFactory: Sized {
    /// Create provider from environment variables and default configuration
    fn from_env() -> super::Result<Self>;

    /// Set or update the model for this provider (builder pattern)
    fn with_model(self, model: &str) -> Self;
}

pub trait StreamingModelProvider: Send + Sync {
    fn stream_response(&self, context: &Context) -> LlmResponseStream;
    fn display_name(&self) -> String;

    /// Context window size in tokens for the current model.
    /// Returns `None` for unknown models (e.g. Ollama, `LlamaCpp`).
    fn context_window(&self) -> Option<u32>;

    /// The `LlmModel` this provider is currently configured to use.
    /// Returns `None` for providers where the model is unknown at compile time
    /// (e.g. test fakes).
    fn model(&self) -> Option<LlmModel> {
        None
    }
}

/// Look up context window for a known provider + model ID combo via the catalog.
///
/// Returns `None` if the model is not in the catalog.
pub fn get_context_window(provider: &str, model_id: &str) -> Option<u32> {
    let key = format!("{provider}:{model_id}");
    key.parse::<LlmModel>()
        .ok()
        .and_then(|m| m.context_window())
}

impl StreamingModelProvider for Box<dyn StreamingModelProvider> {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        (**self).stream_response(context)
    }

    fn display_name(&self) -> String {
        (**self).display_name()
    }

    fn context_window(&self) -> Option<u32> {
        (**self).context_window()
    }

    fn model(&self) -> Option<LlmModel> {
        (**self).model()
    }
}

impl<T: StreamingModelProvider> StreamingModelProvider for std::sync::Arc<T> {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        (**self).stream_response(context)
    }

    fn display_name(&self) -> String {
        (**self).display_name()
    }

    fn context_window(&self) -> Option<u32> {
        (**self).context_window()
    }

    fn model(&self) -> Option<LlmModel> {
        (**self).model()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_context_window_known_model() {
        assert_eq!(
            get_context_window("anthropic", "claude-opus-4-6"),
            Some(1_000_000)
        );
    }

    #[test]
    fn lookup_context_window_openrouter_model() {
        // OpenRouter Qwen models should resolve from catalog
        let result = get_context_window("openrouter", "anthropic/claude-opus-4");
        assert_eq!(result, Some(200_000));
    }

    #[test]
    fn lookup_context_window_unknown_model() {
        assert_eq!(get_context_window("anthropic", "unknown-model-xyz"), None);
    }

    #[test]
    fn lookup_context_window_unknown_provider() {
        assert_eq!(get_context_window("unknown-provider", "some-model"), None);
    }
}
