use color_eyre::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::future::Future;
use std::pin::Pin;
use tokio_stream::Stream;

// Import types from crate::types instead of duplicating
use crate::types::{StreamEvent, ToolCall, ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub temperature: Option<f32>,
}

// Simplified ChatMessage for LLM provider interface (without timestamps)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum ChatMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

pub type StreamEventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>;
pub trait LlmProvider: Send + Sync {
    fn complete_stream_chunks(
        &self,
        request: ChatRequest,
    ) -> impl Future<Output = Result<StreamEventStream>>;
}
