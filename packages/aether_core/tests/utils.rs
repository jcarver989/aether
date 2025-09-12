#![allow(dead_code)]

use aether_core::llm::{ChatRequest, LlmProvider};
use aether_core::mcp::client::McpClient;
use aether_core::mcp::mcp_config::McpServerConfig;
use aether_core::tools::ToolRegistry;
use aether_core::types::{LlmMessage, ToolCall, ToolDefinition};
use color_eyre::Result;
use rmcp::model::Tool as RmcpTool;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_stream::iter;

// Common test configurations
pub const TEST_MODEL: &str = "test-model";
pub const TEST_SERVER_URL: &str = "http://localhost:3000/mcp";
pub const TEST_TOOL_ID: &str = "call_123";

// MCP Test Helpers

pub fn create_test_mcp_client() -> McpClient {
    McpClient::new()
}

pub fn create_test_mcp_server_config(url: &str) -> McpServerConfig {
    McpServerConfig::Http {
        url: url.to_string(),
        headers: HashMap::new(),
    }
}

pub fn create_test_mcp_server_config_with_headers(
    url: &str,
    headers: HashMap<String, String>,
) -> McpServerConfig {
    McpServerConfig::Http {
        url: url.to_string(),
        headers,
    }
}

// Tool Registry Test Helpers

pub fn create_test_rmcp_tool(name: &str, description: &str) -> RmcpTool {
    let mut properties = Map::new();
    properties.insert(
        "path".to_string(),
        json!({"type": "string", "description": "File path"}),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), json!(properties));
    schema.insert("required".to_string(), json!(["path"]));

    RmcpTool::new(name.to_string(), description.to_string(), Arc::new(schema))
}

pub fn create_test_rmcp_tool_with_params(
    name: &str,
    description: &str,
    properties: Map<String, Value>,
    required: Vec<&str>,
) -> RmcpTool {
    let mut schema = Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), json!(properties));
    schema.insert(
        "required".to_string(),
        json!(required.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
    );

    RmcpTool::new(name.to_string(), description.to_string(), Arc::new(schema))
}

pub fn create_test_tool_registry() -> ToolRegistry {
    ToolRegistry::new()
}

pub fn create_test_tool_registry_with_tools(tools: Vec<(&str, &str, &str)>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    for (server, name, description) in tools {
        let tool = create_test_rmcp_tool(name, description);
        registry.register_tool(server.to_string(), tool);
    }
    registry
}

// LLM Provider Test Helpers

pub struct FakeLlmProvider {
    pub chunks: Vec<LlmMessage>,
}

impl FakeLlmProvider {
    pub fn new(chunks: Vec<LlmMessage>) -> Self {
        Self { chunks }
    }

    pub fn with_content(content: &str) -> Self {
        let chunks = vec![
            LlmMessage::Content {
                chunk: content.to_string(),
            },
            LlmMessage::Done,
        ];
        Self { chunks }
    }

    pub fn with_content_chunks(content_chunks: Vec<&str>) -> Self {
        let mut chunks: Vec<LlmMessage> = content_chunks
            .into_iter()
            .map(|s| LlmMessage::Content {
                chunk: s.to_string(),
            })
            .collect();
        chunks.push(LlmMessage::Done);
        Self { chunks }
    }

    pub fn with_tool_call(content: &str, tool_id: &str, tool_name: &str, arguments: &str) -> Self {
        let chunks = vec![
            LlmMessage::Content {
                chunk: content.to_string(),
            },
            LlmMessage::ToolCallStart {
                id: tool_id.to_string(),
                name: tool_name.to_string(),
            },
            LlmMessage::ToolCallArgument {
                id: tool_id.to_string(),
                chunk: arguments.to_string(),
            },
            LlmMessage::ToolCallComplete {
                id: tool_id.to_string(),
            },
            LlmMessage::Done,
        ];
        Self { chunks }
    }

    pub fn with_error_after(content: &str, _chunk_count: usize) -> Self {
        let chunks = vec![LlmMessage::Content {
            chunk: content.to_string(),
        }];
        // Note: Error handling would be implemented in a specialized provider
        Self { chunks }
    }
}

impl LlmProvider for FakeLlmProvider {
    fn complete_stream_chunks(&self, _request: ChatRequest) -> impl tokio_stream::Stream<Item = Result<LlmMessage>> + Send {
        let chunks = self.chunks.clone();
        iter(chunks.into_iter().map(Ok))
    }
}

// Chat Message Test Helpers

pub fn create_test_tool_definition(name: &str, description: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "param": {
                    "type": "string",
                    "description": "A test parameter"
                }
            },
            "required": ["param"]
        })
        .to_string(),
        server: None,
    }
}

pub fn create_test_tool_call(id: &str, name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        arguments: arguments.to_string(),
    }
}

// Stream Processing Test Helpers

pub async fn collect_stream_content(mut stream: impl tokio_stream::Stream<Item = Result<LlmMessage>> + Unpin) -> Result<String> {
    use tokio_stream::StreamExt;

    let mut content = String::new();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        if let LlmMessage::Content { chunk: text } = chunk {
            content.push_str(&text);
        } else if let LlmMessage::Done = chunk {
            break;
        }
    }
    Ok(content)
}

pub async fn collect_stream_chunks(mut stream: impl tokio_stream::Stream<Item = Result<LlmMessage>> + Unpin) -> Result<Vec<LlmMessage>> {
    use tokio_stream::StreamExt;

    let mut chunks = Vec::new();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        let is_done = matches!(chunk, LlmMessage::Done);
        chunks.push(chunk);
        if is_done {
            break;
        }
    }
    Ok(chunks)
}

// JSON Test Helpers

pub fn create_test_json_object(pairs: Vec<(&str, Value)>) -> Value {
    let mut obj = Map::new();
    for (key, value) in pairs {
        obj.insert(key.to_string(), value);
    }
    json!(obj)
}

// JSON Argument Fixing Helper

/// Fix malformed JSON string arguments from LLM models.
/// Some models incorrectly return argument values as JSON strings instead of their actual types.
/// For example: {"query": "[\"value\"]"} instead of {"query": ["value"]}
pub fn fix_json_string_arguments(mut arguments: Value) -> Value {
    if let Some(obj) = arguments.as_object_mut() {
        for (_key, value) in obj.iter_mut() {
            if let Some(string_val) = value.as_str() {
                // Try to parse the string as JSON
                if let Ok(parsed_val) = serde_json::from_str::<Value>(string_val) {
                    // Only replace if the parsed value is not a string (to avoid infinite recursion)
                    match parsed_val {
                        Value::Array(_)
                        | Value::Object(_)
                        | Value::Number(_)
                        | Value::Bool(_)
                        | Value::Null => {
                            *value = parsed_val;
                        }
                        _ => {
                            // If it's still a string, don't replace
                        }
                    }
                }
            }
        }
    }
    arguments
}

// Assertion Helpers

pub fn assert_tool_in_registry(registry: &ToolRegistry, tool_name: &str, expected_server: &str) {
    assert!(
        registry.list_tools().contains(&tool_name.to_string()),
        "Tool '{tool_name}' should be in registry"
    );
    assert_eq!(
        registry.get_server_for_tool(tool_name),
        Some(&expected_server.to_string()),
        "Tool '{tool_name}' should map to server '{expected_server}'"
    );
}

pub fn assert_stream_event_matches(actual: &LlmMessage, expected: &LlmMessage) {
    match (actual, expected) {
        (LlmMessage::Content { chunk: a }, LlmMessage::Content { chunk: b }) => {
            assert_eq!(a, b)
        }
        (
            LlmMessage::ToolCallStart {
                id: id1,
                name: name1,
            },
            LlmMessage::ToolCallStart {
                id: id2,
                name: name2,
            },
        ) => {
            assert_eq!(id1, id2);
            assert_eq!(name1, name2);
        }
        (
            LlmMessage::ToolCallArgument {
                id: id1,
                chunk: arg1,
            },
            LlmMessage::ToolCallArgument {
                id: id2,
                chunk: arg2,
            },
        ) => {
            assert_eq!(id1, id2);
            assert_eq!(arg1, arg2);
        }
        (LlmMessage::ToolCallComplete { id: id1 }, LlmMessage::ToolCallComplete { id: id2 }) => {
            assert_eq!(id1, id2);
        }
        (LlmMessage::Done, LlmMessage::Done) => {}
        _ => panic!("Stream chunk mismatch:\nActual: {actual:?}\nExpected: {expected:?}"),
    }
}
