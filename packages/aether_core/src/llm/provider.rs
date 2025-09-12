use crate::types::{ChatMessage, LlmResponse, ToolDefinition};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio_stream::Stream;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Context {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
}

pub trait ModelProvider: Send + Sync {
    fn generate_response(&self, context: Context)
    -> impl Stream<Item = Result<LlmResponse>> + Send;
}
