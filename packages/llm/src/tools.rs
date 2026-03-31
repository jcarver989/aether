use serde::{Deserialize, Serialize};

#[doc = include_str!("docs/tools.md")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: String,
    pub server: Option<String>,
}

/// Tool call request from the LLM
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Successful result of a tool call
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: String,
}

/// Error result of a tool call
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallError {
    pub id: String,
    pub name: String,
    pub arguments: Option<String>,
    pub error: String,
}

impl ToolCallError {
    pub fn from_request(request: &ToolCallRequest, error: impl Into<String>) -> Self {
        Self {
            id: request.id.clone(),
            name: request.name.clone(),
            arguments: Some(request.arguments.clone()),
            error: error.into(),
        }
    }
}
