use rmcp::model::CallToolRequestParam;
use serde::{Deserialize, Serialize};

use crate::mcp::manager::parse_namespaced_tool_name;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallError {
    pub id: String,
    pub name: String,
    pub arguments: Option<String>,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: String,
    pub server: Option<String>,
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

impl TryFrom<(&ToolCallRequest, rmcp::model::CallToolResult)> for ToolCallResult {
    type Error = ToolCallError;

    fn try_from(
        (request, mcp_result): (&ToolCallRequest, rmcp::model::CallToolResult),
    ) -> Result<Self, Self::Error> {
        if mcp_result.is_error.unwrap_or(false) {
            let error_msg = mcp_result
                .content
                .first()
                .map(|content| format!("{content:?}"))
                .unwrap_or_else(|| "Unknown error".to_string());
            Err(ToolCallError {
                id: request.id.clone(),
                name: request.name.clone(),
                arguments: Some(request.arguments.clone()),
                error: format!("Tool execution error: {}", error_msg),
            })
        } else {
            let result_value = mcp_result
                .content
                .first()
                .map(|content| {
                    serde_json::to_value(content)
                        .unwrap_or(serde_json::Value::String("Serialization error".to_string()))
                })
                .unwrap_or_else(|| serde_json::Value::String("No result".to_string()));
            Ok(ToolCallResult {
                id: request.id.clone(),
                name: request.name.clone(),
                arguments: request.arguments.clone(),
                result: result_value.to_string(),
            })
        }
    }
}
