use std::path::{Path, PathBuf};

use mcp_utils::client::split_on_server_name;
use mcp_utils::display_meta::ToolResultMeta;
use rmcp::model::CallToolRequestParams;
use serde_json;

use llm::{ToolCallError, ToolCallRequest, ToolCallResult};

/// Maximum bytes for a tool result before spilling to disk.
/// ~50K tokens at ~4 bytes/token.
const TOOL_RESULT_MAX_BYTES: usize = 200_000;

/// Size of the head preview included inline when a result spills to disk.
const SPILLOVER_PREVIEW_BYTES: usize = 10_000;

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
/// extracting any `_meta` metadata from structured content.
pub fn mcp_result_to_tool_call_result(
    request: &ToolCallRequest,
    mcp_result: rmcp::model::CallToolResult,
) -> Result<(ToolCallResult, Option<ToolResultMeta>), ToolCallError> {
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
        let (result_value, result_meta) =
            extract_result_and_meta(mcp_result.structured_content, &mcp_result.content);
        // YAML is ~18% more token-efficient than JSON for LLM consumption
        let yaml = serde_yml::to_string(&result_value).unwrap_or_else(|_| result_value.to_string());
        let result_str =
            maybe_spillover(&request.id, yaml, TOOL_RESULT_MAX_BYTES, &spillover_dir());
        Ok((
            ToolCallResult {
                id: request.id.clone(),
                name: request.name.clone(),
                arguments: request.arguments.clone(),
                result: result_str,
            },
            result_meta,
        ))
    }
}

fn spillover_dir() -> PathBuf {
    std::env::temp_dir().join("aether-tool-output")
}

/// If `result` exceeds `max_bytes`, write the full output to disk and return
/// a head preview with a pointer to the file. Otherwise return unchanged.
fn maybe_spillover(tool_id: &str, result: String, max_bytes: usize, dir: &Path) -> String {
    if result.len() <= max_bytes {
        return result;
    }

    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!("Failed to create tool-output dir: {e}");
        return result;
    }

    let file_path = dir.join(format!("{tool_id}.txt"));

    if let Err(e) = std::fs::write(&file_path, &result) {
        tracing::warn!("Failed to write spillover file: {e}");
        return result;
    }

    let preview_end = result.floor_char_boundary(SPILLOVER_PREVIEW_BYTES);
    let preview = &result[..preview_end];
    let total_bytes = result.len();

    format!(
        "<preview>\n{preview}\n</preview>\n\n[Tool result too large ({total_bytes} bytes). Full output saved to {path}. Use grep, read, or tail to explore the full result.]",
        path = file_path.display()
    )
}

fn extract_result_and_meta(
    structured_content: Option<serde_json::Value>,
    content: &[rmcp::model::Content],
) -> (serde_json::Value, Option<ToolResultMeta>) {
    if let Some(mut val) = structured_content {
        let result_meta = extract_result_meta(&mut val);
        (val, result_meta)
    } else {
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

fn extract_result_meta(value: &mut serde_json::Value) -> Option<ToolResultMeta> {
    let obj = value.as_object_mut()?;
    let parsed: ToolResultMeta = {
        let meta = obj.get("_meta")?.as_object()?;
        serde_json::from_value(serde_json::Value::Object(meta.clone())).ok()?
    };

    let meta_empty = {
        let meta = obj.get_mut("_meta")?.as_object_mut()?;
        for key in ["display", "diff_preview", "plan"] {
            meta.remove(key);
        }
        meta.is_empty()
    };

    if meta_empty {
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
    fn test_extracts_and_strips_meta() {
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

        let (result, result_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();

        // _meta should be stripped from the result
        assert!(!result.result.contains("_meta"));
        assert!(result.result.contains("success"));

        // result_meta should be extracted
        let rm = result_meta.expect("result_meta should be present");
        assert_eq!(rm.display.title, "Read file");
        assert_eq!(rm.display.value, "file.rs, 50 lines");
        assert!(rm.diff_preview.is_none());
    }

    #[test]
    fn test_extracts_meta_with_diff_preview() {
        let request = make_request();

        let structured = json!({
            "status": "success",
            "_meta": {
                "display": {
                    "title": "Edit file",
                    "value": "main.rs"
                },
                "diff_preview": {
                    "removed": ["old line"],
                    "added": ["new line"],
                    "lang_hint": "rs"
                }
            }
        });

        let mcp_result = McpCallToolResult {
            content: vec![],
            structured_content: Some(structured),
            is_error: Some(false),
            meta: None,
        };

        let (result, result_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();
        assert!(!result.result.contains("_meta"));

        let rm = result_meta.expect("result_meta should be present");
        assert_eq!(rm.display.title, "Edit file");
        let dp = rm.diff_preview.expect("diff_preview should be present");
        assert_eq!(dp.removed, vec!["old line"]);
        assert_eq!(dp.added, vec!["new line"]);
        assert_eq!(dp.lang_hint, "rs");
    }

    #[test]
    fn test_extracts_known_meta_and_preserves_unknown_meta_keys() {
        let request = make_request();

        let structured = json!({
            "status": "success",
            "_meta": {
                "display": {
                    "title": "Edit file",
                    "value": "main.rs"
                },
                "diff_preview": {
                    "removed": ["old line"],
                    "added": ["new line"],
                    "lang_hint": "rs"
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

        let (result, result_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();
        let rm = result_meta.expect("result_meta should be present");
        assert_eq!(rm.display.title, "Edit file");
        assert!(rm.diff_preview.is_some());
        assert!(!result.result.contains("display:"));
        assert!(!result.result.contains("diff_preview:"));
        assert!(result.result.contains("trace_id:"));
        assert!(result.result.contains("trace-123"));
        assert!(result.result.contains("duration_ms:"));
        assert!(result.result.contains("18"));
    }

    #[test]
    fn test_malformed_meta_returns_none() {
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

        let (result, result_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();
        assert!(result_meta.is_none());
        assert!(result.result.contains("display:"));
        assert!(result.result.contains("not a valid ToolDisplayMeta"));
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

        let (result, result_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();
        assert!(result.result.contains("success"));
        assert!(result.result.contains("hello"));
        assert!(result_meta.is_none());
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

        let (result, result_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();
        assert!(result.result.contains("plain text result"));
        assert!(result_meta.is_none());
    }

    /// Regression test: verifies that a struct with `#[serde(rename_all = "camelCase")]`
    /// correctly serializes `_meta` as `"_meta"` (not `"Meta"`) when `#[serde(rename = "_meta")]`
    /// is present, so `extract_result_meta` can find it.
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

        // Verify extract_result_and_meta can find it
        let (stripped, result_meta) = extract_result_and_meta(Some(serialized), &[]);
        let rm = result_meta.expect("result_meta should be extracted from serialized struct");
        assert_eq!(rm.display.title, "Read file");
        assert_eq!(rm.display.value, "file.rs, 50 lines");

        // _meta should be stripped from the result
        assert!(stripped.get("_meta").is_none());
    }

    #[test]
    fn test_extracts_meta_with_plan() {
        let request = make_request();

        let structured = json!({
            "status": "success",
            "_meta": {
                "display": {
                    "title": "Todo",
                    "value": "Research AI agents"
                },
                "plan": {
                    "entries": [
                        { "content": "Research AI agents", "status": "in_progress" },
                        { "content": "Write tests", "status": "pending" }
                    ]
                }
            }
        });

        let mcp_result = McpCallToolResult {
            content: vec![],
            structured_content: Some(structured),
            is_error: Some(false),
            meta: None,
        };

        let (result, result_meta) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();
        assert!(!result.result.contains("_meta"));

        let rm = result_meta.expect("result_meta should be present");
        assert_eq!(rm.display.title, "Todo");
        let plan = rm.plan.expect("plan should be present");
        assert_eq!(plan.entries.len(), 2);
        assert_eq!(plan.entries[0].content, "Research AI agents");
        assert_eq!(
            plan.entries[0].status,
            mcp_utils::display_meta::PlanMetaStatus::InProgress
        );
        assert_eq!(
            plan.entries[1].status,
            mcp_utils::display_meta::PlanMetaStatus::Pending
        );
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
        let (_, result_meta) = extract_result_and_meta(Some(serialized), &[]);
        assert!(
            result_meta.is_none(),
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

    #[test]
    fn test_result_is_yaml_format() {
        let request = make_request();

        let structured = json!({
            "status": "success",
            "files": [
                {"name": "Cargo.toml", "path": "./Cargo.toml"},
                {"name": "src", "path": "./src"}
            ],
            "totalCount": 2
        });

        let mcp_result = McpCallToolResult {
            content: vec![],
            structured_content: Some(structured),
            is_error: Some(false),
            meta: None,
        };

        let (result, _) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();

        // YAML uses unquoted keys with colons, not JSON braces/quotes
        assert!(
            result.result.contains("status: success"),
            "expected YAML key: value format, got: {}",
            result.result
        );
        assert!(
            result.result.contains("totalCount: 2"),
            "expected YAML key: value format, got: {}",
            result.result
        );
        // YAML lists use `- ` prefix
        assert!(
            result.result.contains("- name:"),
            "expected YAML list items, got: {}",
            result.result
        );
        // Should NOT contain JSON braces/brackets at the top level
        assert!(
            !result.result.starts_with('{'),
            "expected YAML, not JSON: {}",
            result.result
        );
    }

    #[test]
    fn test_serde_yml_produces_yaml_not_json() {
        let value = json!({"key": "value"});
        let yaml = serde_yml::to_string(&value).unwrap();
        assert!(yaml.contains("key:"));
        assert!(yaml.contains("value"));
        assert!(!yaml.starts_with('{'));
    }

    #[test]
    fn test_spillover_small_input_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let input = "hello world".to_string();
        let result = maybe_spillover("test_small", input.clone(), 1000, dir.path());
        assert_eq!(result, input);
    }

    #[test]
    fn test_spillover_large_input_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let large = "x".repeat(5000);
        let result = maybe_spillover("test_large_write", large.clone(), 1000, dir.path());

        assert!(result.contains("<preview>"));
        assert!(result.contains("</preview>"));
        assert!(result.contains("Tool result too large"));
        assert!(result.contains("5000 bytes"));
        assert!(result.contains("test_large_write.txt"));

        let file_path = dir.path().join("test_large_write.txt");
        assert!(file_path.exists());
        let on_disk = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(on_disk, large);
    }

    #[test]
    fn test_spillover_preview_content() {
        let dir = tempfile::tempdir().unwrap();
        let head = "HEAD_CONTENT_";
        let tail_marker = "TAIL_MARKER";
        let padding = "z".repeat(SPILLOVER_PREVIEW_BYTES + 5000);
        let large = format!("{head}{padding}{tail_marker}");
        let result = maybe_spillover("test_preview", large, 1000, dir.path());

        assert!(result.contains(head));
        assert!(!result.contains(tail_marker));
    }

    #[test]
    fn test_spillover_preserves_utf8_boundaries() {
        let dir = tempfile::tempdir().unwrap();
        let emoji_line = "\u{1F600}".repeat(300); // 1200 bytes of emoji
        let large = format!("{}{}", emoji_line, "a".repeat(5000));

        let result = maybe_spillover("test_utf8", large, 100, dir.path());

        assert!(result.contains("<preview>"));

        let preview_start = result.find("<preview>\n").unwrap() + "<preview>\n".len();
        let preview_end = result.find("\n</preview>").unwrap();
        let preview = &result[preview_start..preview_end];
        assert!(preview.chars().count() > 0);
    }

    #[test]
    fn test_mcp_result_spills_large_output() {
        let request = ToolCallRequest {
            id: "spill_integration".to_string(),
            name: "big_tool".to_string(),
            arguments: "{}".to_string(),
        };

        let big_value = "x".repeat(TOOL_RESULT_MAX_BYTES + 1000);
        let structured = json!({ "data": big_value });

        let mcp_result = McpCallToolResult {
            content: vec![],
            structured_content: Some(structured),
            is_error: Some(false),
            meta: None,
        };

        let (result, _) = mcp_result_to_tool_call_result(&request, mcp_result).unwrap();

        assert!(result.result.contains("<preview>"));
        assert!(result.result.contains("Tool result too large"));
        assert!(result.result.contains("spill_integration.txt"));
    }
}
