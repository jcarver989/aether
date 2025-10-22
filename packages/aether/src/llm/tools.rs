use rmcp::model::CallToolRequestParam;
use serde::{Deserialize, Serialize};

use crate::mcp::manager::split_on_server_name;

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

/// Progress information for a tool call
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCallProgress {
    pub progress: f64,
    pub total: Option<f64>,
    pub message: Option<String>,
}

/// Status updates for tool execution
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolCallStatus {
    /// Tool execution has started
    Started {
        id: String,
        name: String,
    },
    /// Tool execution is in progress
    InProgress {
        id: String,
        progress: ToolCallProgress,
    },
    /// Tool execution completed successfully
    Complete {
        result: ToolCallResult,
    },
    /// Tool execution failed
    Error {
        error: ToolCallError,
    },
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
        let tool_name = split_on_server_name(&request.name)
            .map(|(_, tool_name)| tool_name.to_string())
            .unwrap_or_else(|| request.name.clone());

        // Parse arguments from JSON string
        let arguments = serde_json::from_str::<serde_json::Value>(&request.arguments)
            .map_err(|e| format!("Invalid tool arguments: {e}"))?
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
                error: format!("Tool execution error: {error_msg}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_call_status_started_serialization() {
        let status = ToolCallStatus::Started {
            id: "test-id".to_string(),
            name: "test-tool".to_string(),
        };

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["Started"]["id"], "test-id");
        assert_eq!(json["Started"]["name"], "test-tool");

        let deserialized: ToolCallStatus = serde_json::from_value(json).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_tool_call_status_in_progress_serialization() {
        let progress = ToolCallProgress {
            progress: 50.0,
            total: Some(100.0),
            message: Some("Processing items...".to_string()),
        };

        let status = ToolCallStatus::InProgress {
            id: "test-id".to_string(),
            progress: progress.clone(),
        };

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["InProgress"]["id"], "test-id");
        assert_eq!(json["InProgress"]["progress"]["progress"], 50.0);
        assert_eq!(json["InProgress"]["progress"]["total"], 100.0);
        assert_eq!(json["InProgress"]["progress"]["message"], "Processing items...");

        let deserialized: ToolCallStatus = serde_json::from_value(json).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_tool_call_status_complete_serialization() {
        let result = ToolCallResult {
            id: "test-id".to_string(),
            name: "test-tool".to_string(),
            arguments: "{}".to_string(),
            result: "{\"status\":\"ok\"}".to_string(),
        };

        let status = ToolCallStatus::Complete {
            result: result.clone(),
        };

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["Complete"]["result"]["id"], "test-id");

        let deserialized: ToolCallStatus = serde_json::from_value(json).unwrap();
        if let ToolCallStatus::Complete { result: r } = deserialized {
            assert_eq!(r.id, result.id);
            assert_eq!(r.name, result.name);
        } else {
            panic!("Expected Complete variant");
        }
    }

    #[test]
    fn test_tool_call_status_error_serialization() {
        let error = ToolCallError {
            id: "test-id".to_string(),
            name: "test-tool".to_string(),
            arguments: Some("{}".to_string()),
            error: "Something went wrong".to_string(),
        };

        let status = ToolCallStatus::Error {
            error: error.clone(),
        };

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["Error"]["error"]["id"], "test-id");
        assert_eq!(json["Error"]["error"]["error"], "Something went wrong");

        let deserialized: ToolCallStatus = serde_json::from_value(json).unwrap();
        if let ToolCallStatus::Error { error: e } = deserialized {
            assert_eq!(e.id, error.id);
            assert_eq!(e.error, error.error);
        } else {
            panic!("Expected Error variant");
        }
    }

    #[test]
    fn test_tool_call_progress_without_total() {
        let progress = ToolCallProgress {
            progress: 42.5,
            total: None,
            message: Some("Unknown total".to_string()),
        };

        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["progress"], 42.5);
        assert_eq!(json["total"], serde_json::Value::Null);
        assert_eq!(json["message"], "Unknown total");

        let deserialized: ToolCallProgress = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.progress, 42.5);
        assert_eq!(deserialized.total, None);
    }

    #[test]
    fn test_tool_call_progress_with_floating_point() {
        let progress = ToolCallProgress {
            progress: 33.33,
            total: Some(99.99),
            message: None,
        };

        let json = serde_json::to_value(&progress).unwrap();
        let deserialized: ToolCallProgress = serde_json::from_value(json).unwrap();

        assert!((deserialized.progress - 33.33).abs() < 0.001);
        assert_eq!(deserialized.total, Some(99.99));
    }
}
