// Re-export rmcp types for convenience
pub use rmcp::model::{
    Tool, CallToolRequestParam, CallToolRequest,
    ListToolsRequest,
};

use serde_json::Value;
use thiserror::Error;

/// Application-specific error types for MCP operations
#[derive(Debug, Error)]
pub enum McpError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    
    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),
    
    #[error("Server initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Transport error: {0}")]
    TransportError(#[from] rmcp::ErrorData),
}

/// Application result type for MCP operations
pub type McpResult<T> = Result<T, McpError>;

/// Helper functions for common MCP operations
pub mod helpers {
    use super::*;
    use serde_json::json;
    
    /// Create tool call arguments from key-value pairs
    pub fn create_tool_args(args: &[(&str, Value)]) -> Value {
        let mut map = serde_json::Map::new();
        for (key, value) in args {
            map.insert(key.to_string(), value.clone());
        }
        Value::Object(map)
    }
    
    /// Extract text content from tool result  
    pub fn extract_text_result(result: &Value) -> Option<String> {
        result.get("content")
            .and_then(|content| content.get(0))
            .and_then(|content| content.get("text"))
            .and_then(|text| text.as_str())
            .map(|s| s.to_string())
    }
    
    /// Check if tool result contains an error
    pub fn has_error(result: &Value) -> bool {
        result.get("is_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    
    /// Get error message from tool result
    pub fn get_error_message(result: &Value) -> Option<String> {
        if has_error(result) {
            extract_text_result(result)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_create_tool_args() {
        let args = helpers::create_tool_args(&[
            ("path", json!("/tmp/test.txt")),
            ("content", json!("Hello, world!")),
        ]);
        
        assert_eq!(args["path"], "/tmp/test.txt");
        assert_eq!(args["content"], "Hello, world!");
    }
    
    #[test]
    fn test_extract_text_result() {
        let result = json!({
            "content": [{"text": "Hello from tool"}],
            "is_error": false
        });
        
        let text = helpers::extract_text_result(&result);
        assert_eq!(text, Some("Hello from tool".to_string()));
    }
    
    #[test]
    fn test_has_error() {
        let result_with_error = json!({
            "content": [{"text": "Something went wrong"}],
            "is_error": true
        });
        
        let result_without_error = json!({
            "content": [{"text": "Success"}],
            "is_error": false
        });
        
        assert!(helpers::has_error(&result_with_error));
        assert!(!helpers::has_error(&result_without_error));
    }
}