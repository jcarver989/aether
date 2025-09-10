use color_eyre::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio_stream::Stream;

// Import types from crate::types instead of duplicating
use crate::types::{ChatMessage, StreamEvent, ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
}

pub trait LlmProvider: Send + Sync {
    fn complete_stream_chunks(
        &self,
        request: ChatRequest,
    ) -> impl Stream<Item = Result<StreamEvent>> + Send;
}
