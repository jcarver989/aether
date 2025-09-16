use serde::{Deserialize, Serialize};

/// A newtype wrapper for ISO 8601 timestamp strings
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsoString(pub String);

impl IsoString {
    /// Create a new IsoString from the current time
    pub fn now() -> Self {
        Self(chrono::Utc::now().to_rfc3339())
    }

    /// Create an IsoString from a chrono DateTime
    pub fn from_datetime<Tz: chrono::TimeZone>(datetime: chrono::DateTime<Tz>) -> Self
    where
        Tz::Offset: std::fmt::Display,
    {
        Self(datetime.to_rfc3339())
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatMessage {
    System {
        content: String,
        timestamp: IsoString,
    },
    User {
        content: String,
        timestamp: IsoString,
    },
    Assistant {
        content: String,
        timestamp: IsoString,
        tool_calls: Vec<ToolCallRequest>,
    },
    AssistantStreaming {
        content: String,
        timestamp: IsoString,
    },
    ToolCallResult {
        tool_call_id: String,
        content: String,
        timestamp: IsoString,
    },
    Error {
        message: String,
        timestamp: IsoString,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCallState {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LlmResponse {
    Start { message_id: String },
    Text { chunk: String },
    ToolRequestStart { id: String, name: String },
    ToolRequestArg { id: String, chunk: String },
    ToolRequestComplete { tool_call: ToolCallRequest },
    Done,
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToolDiscoveryEvent {
    Discovered { tool: ToolDefinition },
    Complete { count: u32 },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: String,
    pub server: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmProvider {
    OpenRouter,
    Ollama,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub provider: ProviderStatus,
    pub mcp_servers: std::collections::HashMap<String, McpServerStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub connected: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerStatus {
    pub connected: bool,
    pub error: Option<String>,
    pub tool_count: u32,
}
