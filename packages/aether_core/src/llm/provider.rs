use async_trait::async_trait;
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::pin::Pin;
use tokio_stream::Stream;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub temperature: Option<f32>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamChunk {
    Content { content: String },
    ToolCallStart { id: String, name: String },
    ToolCallArgument { id: String, argument: String },
    ToolCallComplete { id: String },
    Done,
}

pub type StreamChunkStream = Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete_stream_chunks(&self, request: ChatRequest) -> Result<StreamChunkStream>;
}

// Implement LlmProvider for Box<dyn LlmProvider> to enable trait object usage
#[async_trait]
impl LlmProvider for Box<dyn LlmProvider> {
    async fn complete_stream_chunks(&self, request: ChatRequest) -> Result<StreamChunkStream> {
        self.as_ref().complete_stream_chunks(request).await
    }
}
