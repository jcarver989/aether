use rmcp::model::CallToolRequestParams;
use serde_json;

use super::manager::split_on_server_name;
use crate::{ToolCallError, ToolCallRequest, ToolCallResult};

/// Convert a ToolCallRequest to rmcp::CallToolRequestParams
pub fn tool_call_request_to_mcp(
    request: &ToolCallRequest,
) -> Result<CallToolRequestParams, String> {
    // Parse the tool name to remove namespace prefix if present
    let tool_name = split_on_server_name(&request.name)
        .map(|(_, tool_name)| tool_name.to_string())
        .unwrap_or_else(|| request.name.clone());

    // Parse arguments from JSON string
    let arguments = serde_json::from_str::<serde_json::Value>(&request.arguments)
        .map_err(|e| format!("Invalid tool arguments: {e}"))?
        .as_object()
        .cloned();

    Ok(CallToolRequestParams {
        meta: None,
        name: tool_name.into(),
        arguments,
        task: None,
    })
}

/// Convert an rmcp CallToolResult and request to ToolCallResult or ToolCallError
pub fn mcp_result_to_tool_call_result(
    request: &ToolCallRequest,
    mcp_result: rmcp::model::CallToolResult,
) -> Result<ToolCallResult, ToolCallError> {
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
            error: format!("Tool execution error: {error_msg}"),
        })
    } else {
        let result_value = mcp_result
            .structured_content
            .or_else(|| {
                mcp_result.content.first().map(|content| {
                    serde_json::to_value(content)
                        .unwrap_or(serde_json::Value::String("Serialization error".to_string()))
                })
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

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{CallToolResult as McpCallToolResult, Content};
    use serde_json::json;

    fn make_request() -> ToolCallRequest {
        ToolCallRequest {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        }
    }

    #[test]
    fn test_tool_call_result_prefers_structured_content() {
        let request = make_request();

        let structured = json!({
            "status": "success",
            "file_path": "/test/file.rs",
            "_meta": {
                "display": {
                    "type": "ReadFile",
                    "filePath": "/test/file.rs",
                    "size": 1024,
                    "lines": 50
                }
            }
        });

        let mcp_result = McpCallToolResult {
            content: vec![Content::text("plain text fallback")],
            structured_content: Some(structured.clone()),
            is_error: Some(false),
            meta: None,
        };

        let result = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();

        assert!(result.result.contains("_meta"));
        assert!(result.result.contains("ReadFile"));
    }

    #[test]
    fn test_tool_call_result_falls_back_to_content() {
        let request = make_request();

        let mcp_result = McpCallToolResult {
            content: vec![Content::text("plain text result")],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let result = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();

        assert!(result.result.contains("plain text result"));
    }

    #[test]
    fn test_tool_call_result_handles_error() {
        let request = make_request();

        let mcp_result = McpCallToolResult {
            content: vec![Content::text("Error: file not found")],
            structured_content: None,
            is_error: Some(true),
            meta: None,
        };

        let result = mcp_result_to_tool_call_result(&request, mcp_result);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.error.contains("file not found"));
    }
}
