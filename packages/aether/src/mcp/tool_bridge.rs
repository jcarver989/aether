use mcp_utils::client::manager::split_on_server_name;
use mcp_utils::display_meta::ToolDisplayMeta;
use rmcp::model::CallToolRequestParams;
use serde_json;

use llm::{ToolCallError, ToolCallRequest, ToolCallResult};

/// Convert a `ToolCallRequest` to `rmcp::CallToolRequestParams`
pub fn tool_call_request_to_mcp(
    request: &ToolCallRequest,
) -> Result<CallToolRequestParams, String> {
    // Parse the tool name to remove namespace prefix if present
    let tool_name = split_on_server_name(&request.name).map_or_else(
        || request.name.clone(),
        |(_, tool_name)| tool_name.to_string(),
    );

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

/// Convert an rmcp `CallToolResult` and request to `ToolCallResult` or `ToolCallError`,
/// extracting any `_meta.display` metadata from structured content.
pub fn mcp_result_to_tool_call_result(
    request: &ToolCallRequest,
    mcp_result: rmcp::model::CallToolResult,
) -> Result<(ToolCallResult, Option<ToolDisplayMeta>), ToolCallError> {
    if mcp_result.is_error.unwrap_or(false) {
        let error_msg = mcp_result.content.first().map_or_else(
            || "Unknown error".to_string(),
            |content| format!("{content:?}"),
        );
        Err(ToolCallError {
            id: request.id.clone(),
            name: request.name.clone(),
            arguments: Some(request.arguments.clone()),
            error: format!("Tool execution error: {error_msg}"),
        })
    } else {
        let (result_value, display_meta) =
            extract_result_and_display_meta(mcp_result.structured_content, &mcp_result.content);
        Ok((
            ToolCallResult {
                id: request.id.clone(),
                name: request.name.clone(),
                arguments: request.arguments.clone(),
                result: result_value.to_string(),
            },
            display_meta,
        ))
    }
}

fn extract_result_and_display_meta(
    structured_content: Option<serde_json::Value>,
    content: &[rmcp::model::Content],
) -> (serde_json::Value, Option<ToolDisplayMeta>) {
    match structured_content {
        Some(mut val) => {
            let display_meta = extract_display_meta(&mut val);
            (val, display_meta)
        }
        None => {
            let fallback = content.first().map_or_else(
                || serde_json::Value::String("No result".to_string()),
                |c| {
                    serde_json::to_value(c)
                        .unwrap_or(serde_json::Value::String("Serialization error".to_string()))
                },
            );
            (fallback, None)
        }
    }
}

fn extract_display_meta(value: &mut serde_json::Value) -> Option<ToolDisplayMeta> {
    let display = value
        .as_object()?
        .get("_meta")?
        .as_object()?
        .get("display")?
        .clone();
    let parsed = serde_json::from_value(display).ok()?;

    let obj = value.as_object_mut()?;
    let meta_obj = obj.get_mut("_meta")?.as_object_mut()?;
    meta_obj.remove("display");
    if meta_obj.is_empty() {
        obj.remove("_meta");
    }

    Some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{CallToolResult as McpCallToolResult, Content};
    use serde::Serialize;
    use serde_json::json;

    fn make_request() -> ToolCallRequest {
        ToolCallRequest {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        }
    }

    #[test]
    fn test_extracts_and_strips_display_meta() {
        let request = make_request();

        let structured = json!({
            "status": "success",
            "file_path": "/test/file.rs",
            "_meta": {
                "display": {
                    "title": "Read file",
                    "value": "file.rs, 50 lines"
                }
            }
        });

        let mcp_result = McpCallToolResult {
            content: vec![Content::text("plain text fallback")],
            structured_content: Some(structured),
            is_error: Some(false),
            meta: None,
        };

        let (result, display_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();

        // _meta should be stripped from the result
        assert!(!result.result.contains("_meta"));
        assert!(result.result.contains("success"));

        // display_meta should be extracted
        let dm = display_meta.expect("display_meta should be present");
        assert_eq!(dm.title, "Read file");
        assert_eq!(dm.value, "file.rs, 50 lines");
    }

    #[test]
    fn test_preserves_non_display_meta_keys() {
        let request = make_request();

        let structured = json!({
            "status": "success",
            "_meta": {
                "display": {
                    "title": "Read file",
                    "value": "file.rs, 50 lines"
                },
                "trace_id": "trace-123",
                "duration_ms": 18
            }
        });

        let mcp_result = McpCallToolResult {
            content: vec![],
            structured_content: Some(structured),
            is_error: Some(false),
            meta: None,
        };

        let (result, display_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();

        assert!(
            display_meta.is_some(),
            "display metadata should be extracted"
        );
        assert!(result.result.contains("\"trace_id\":\"trace-123\""));
        assert!(result.result.contains("\"duration_ms\":18"));
        assert!(!result.result.contains("\"display\""));
    }

    #[test]
    fn test_malformed_display_meta_returns_none() {
        let request = make_request();

        let structured = json!({
            "status": "success",
            "_meta": {
                "display": "not a valid ToolDisplayMeta"
            }
        });

        let mcp_result = McpCallToolResult {
            content: vec![],
            structured_content: Some(structured),
            is_error: Some(false),
            meta: None,
        };

        let (result, display_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();
        assert!(display_meta.is_none());
        assert!(
            result
                .result
                .contains("\"display\":\"not a valid ToolDisplayMeta\""),
            "malformed display metadata should remain in the result payload"
        );
    }

    #[test]
    fn test_no_meta_passes_through_unchanged() {
        let request = make_request();

        let structured = json!({
            "status": "success",
            "data": "hello"
        });

        let mcp_result = McpCallToolResult {
            content: vec![],
            structured_content: Some(structured),
            is_error: Some(false),
            meta: None,
        };

        let (result, display_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();
        assert!(result.result.contains("success"));
        assert!(result.result.contains("hello"));
        assert!(display_meta.is_none());
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

        let (result, display_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();
        assert!(result.result.contains("plain text result"));
        assert!(display_meta.is_none());
    }

    /// Regression test: verifies that a struct with `#[serde(rename_all = "camelCase")]`
    /// correctly serializes `_meta` as `"_meta"` (not `"Meta"`) when `#[serde(rename = "_meta")]`
    /// is present, so `extract_result_and_display_meta` can find it.
    #[test]
    fn test_meta_survives_serde_round_trip_with_camel_case_rename() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct FakeToolResult {
            file_path: String,
            total_lines: usize,
            #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
            _meta: Option<serde_json::Value>,
        }

        let result = FakeToolResult {
            file_path: "/test/file.rs".to_string(),
            total_lines: 50,
            _meta: Some(json!({
                "display": {
                    "title": "Read file",
                    "value": "file.rs, 50 lines"
                }
            })),
        };

        let serialized = serde_json::to_value(&result).unwrap();

        // The key assertion: `_meta` must appear as "_meta" in the JSON, not "Meta"
        assert!(
            serialized.get("_meta").is_some(),
            "expected `_meta` key in serialized JSON, got: {serialized}"
        );

        // Verify extract_result_and_display_meta can find it
        let (stripped, display_meta) = extract_result_and_display_meta(Some(serialized), &[]);
        let dm = display_meta.expect("display_meta should be extracted from serialized struct");
        assert_eq!(dm.title, "Read file");
        assert_eq!(dm.value, "file.rs, 50 lines");

        // _meta should be stripped from the result
        assert!(stripped.get("_meta").is_none());
    }

    /// Demonstrates the bug: without `#[serde(rename = "_meta")]`, camelCase
    /// converts `_meta` to `"Meta"`, making it invisible to extraction.
    #[test]
    fn test_meta_without_rename_breaks_extraction() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct BrokenToolResult {
            file_path: String,
            // Intentionally missing `#[serde(rename = "_meta")]`
            #[serde(skip_serializing_if = "Option::is_none")]
            _meta: Option<serde_json::Value>,
        }

        let result = BrokenToolResult {
            file_path: "/test/file.rs".to_string(),
            _meta: Some(json!({
                "display": {
                    "title": "Read file",
                    "value": "file.rs, 50 lines"
                }
            })),
        };

        let serialized = serde_json::to_value(&result).unwrap();

        // Without the rename, camelCase mangles `_meta` into "meta"
        assert!(
            serialized.get("_meta").is_none(),
            "without #[serde(rename)], _meta should be mangled by camelCase"
        );
        assert!(serialized.get("meta").is_some());

        // Extraction fails — this is the bug we fixed
        let (_, display_meta) = extract_result_and_display_meta(Some(serialized), &[]);
        assert!(
            display_meta.is_none(),
            "extraction should fail when _meta is mangled to Meta"
        );
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

        let err = mcp_result_to_tool_call_result(&request, mcp_result).unwrap_err();
        assert!(err.error.contains("file not found"));
    }
}
