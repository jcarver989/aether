use serde::{Deserialize, Serialize};
use specta::Type;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ChatMessage {
    System {
        content: String,
        timestamp: SystemTime,
    },
    User {
        content: String,
        timestamp: SystemTime,
    },
    Assistant {
        content: String,
        timestamp: SystemTime,
    },
    AssistantStreaming {
        content: String,
        timestamp: SystemTime,
    },
    Tool {
        tool_call_id: String,
        content: String,
        timestamp: SystemTime,
    },
    ToolCall {
        id: String,
        name: String,
        params: String,
        timestamp: SystemTime,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
        timestamp: SystemTime,
    },
    Error {
        message: String,
        timestamp: SystemTime,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ToolCallState {
    Pending,
    Running,
    Completed,
    Failed,
}
