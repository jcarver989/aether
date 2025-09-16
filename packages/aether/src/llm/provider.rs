use crate::types::{ChatMessage, LlmResponse, ToolDefinition};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio_stream::Stream;

// We use Box<dyn> here instead of impl Stream primarily to support a nicer user-facing API for
// alloyed models -- i.e. it allows us to have Vec<Box<dyn ModelProvider>> in AlloyedModelProvider
pub type LlmResponseStream = Pin<Box<dyn Stream<Item = Result<LlmResponse>> + Send>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
}

pub trait ModelProvider: Send + Sync {
    fn stream_response(&self, context: Context) -> LlmResponseStream;
    fn display_name(&self) -> String;
}

impl ModelProvider for Box<dyn ModelProvider> {
    fn stream_response(&self, context: Context) -> LlmResponseStream {
        (**self).stream_response(context)
    }

    fn display_name(&self) -> String {
        (**self).display_name()
    }
}
