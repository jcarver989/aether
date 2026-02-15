use crate::Result as LlmResult;
use std::pin::Pin;
use tokio_stream::Stream;

use super::{Context, LlmResponse};

// We use Box<dyn> here instead of impl Stream primarily to support a nicer user-facing API for
// alloyed models -- i.e. it allows us to have Vec<Box<dyn ModelProvider>> in AlloyedModelProvider
pub type LlmResponseStream = Pin<Box<dyn Stream<Item = LlmResult<LlmResponse>> + Send>>;

/// Factory trait for constructing model providers
///
/// This trait is separate from StreamingModelProvider to allow trait objects
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
}

impl StreamingModelProvider for Box<dyn StreamingModelProvider> {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        (**self).stream_response(context)
    }

    fn display_name(&self) -> String {
        (**self).display_name()
    }
}

impl<T: StreamingModelProvider> StreamingModelProvider for std::sync::Arc<T> {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        (**self).stream_response(context)
    }

    fn display_name(&self) -> String {
        (**self).display_name()
    }
}
