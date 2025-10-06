use std::fmt::Display;

use chrono::{DateTime, TimeZone};
use serde::{Deserialize, Serialize};
use rmcp::model::CallToolRequestParam;

use crate::mcp::manager::parse_namespaced_tool_name;

/// A newtype wrapper for ISO 8601 timestamp strings
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsoString(pub String);

impl IsoString {
    /// Create a new IsoString from the current time
    pub fn now() -> Self {
        Self(chrono::Utc::now().to_rfc3339())
    }

    /// Create an IsoString from a chrono DateTime
    pub fn from_datetime<T: TimeZone>(datetime: DateTime<T>) -> Self
    where
        T::Offset: Display,
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

impl TryFrom<&ToolCallRequest> for CallToolRequestParam {
    type Error = String;

    fn try_from(request: &ToolCallRequest) -> Result<Self, Self::Error> {
        // Parse the tool name to remove namespace prefix if present
        let tool_name = parse_namespaced_tool_name(&request.name)
            .map(|(_, tool_name)| tool_name.to_string())
            .unwrap_or_else(|| request.name.clone());

        // Parse arguments from JSON string
        let arguments = serde_json::from_str::<serde_json::Value>(&request.arguments)
            .map_err(|e| format!("Invalid tool arguments: {}", e))?
            .as_object()
            .cloned();

        Ok(CallToolRequestParam {
            name: tool_name.into(),
            arguments,
        })
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
pub enum LlmProvider {
    Anthropic,
    OpenRouter,
    Ollama,
    LlamaCpp,
}

impl LlmProvider {
    pub fn from_str(provider: &str) -> Result<LlmProvider, String> {
        match provider {
            "anthropic" => Ok(LlmProvider::Anthropic),
            "openrouter" => Ok(LlmProvider::OpenRouter),
            "ollama" => Ok(LlmProvider::Ollama),
            "llamacpp" => Ok(LlmProvider::LlamaCpp),
            _ => Err(format!(
                "Unknown provider: {}. Supported providers: anthropic, openrouter, ollama, llamacpp",
                provider
            )),
        }
    }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub enable_prompt_caching: Option<bool>,
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
