use anyhow::Result;
use aether::config::{Config, McpServerConfig};
use aether::mcp::McpClient;
use std::collections::HashMap;
use tokio;

#[tokio::test]
async fn test_mcp_client_creation() {
    let client = McpClient::new();
    assert!(client.get_available_tools().is_empty());
}

#[tokio::test]
async fn test_mcp_server_config() {
    let config = McpServerConfig {
        url: "http://localhost:3000/mcp".to_string(),
        headers: HashMap::new(),
    };
    
    assert_eq!(config.url, "http://localhost:3000/mcp");
    assert!(config.headers.is_empty());
}

#[tokio::test]
async fn test_mcp_server_config_with_headers() {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    
    let config = McpServerConfig {
        url: "https://api.example.com/mcp".to_string(),
        headers,
    };
    
    assert_eq!(config.url, "https://api.example.com/mcp");
    assert_eq!(config.headers.len(), 2);
    assert_eq!(config.headers.get("Authorization"), Some(&"Bearer token123".to_string()));
    assert_eq!(config.headers.get("Content-Type"), Some(&"application/json".to_string()));
}

// Note: These tests don't actually connect to real MCP servers
// In a real scenario, you would need running MCP servers to test against
// For now, these tests validate the basic structure and configuration