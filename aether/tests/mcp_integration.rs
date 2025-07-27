use anyhow::Result;
use aether::mcp::McpClient;
use aether::mcp_config::McpServerConfig;
use std::collections::HashMap;

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

#[tokio::test]
async fn test_mcp_server_config_serialization() -> Result<()> {
    let mut headers = HashMap::new();
    headers.insert("X-API-Key".to_string(), "secret123".to_string());
    
    let config = McpServerConfig {
        url: "https://mcp.example.com".to_string(),
        headers,
    };
    
    let serialized = serde_json::to_string(&config)?;
    let deserialized: McpServerConfig = serde_json::from_str(&serialized)?;
    
    assert_eq!(deserialized.url, "https://mcp.example.com");
    assert_eq!(deserialized.headers.len(), 1);
    assert_eq!(deserialized.headers.get("X-API-Key"), Some(&"secret123".to_string()));
    
    Ok(())
}

#[tokio::test]
async fn test_mcp_client_tool_registry() {
    let client = McpClient::new();
    
    // Test that tool registry starts empty
    assert_eq!(client.get_available_tools().len(), 0);
    assert!(client.get_tool_description("nonexistent").is_none());
}

#[test]
fn test_mcp_server_config_with_empty_headers() {
    let config = McpServerConfig {
        url: "http://localhost:8080/mcp".to_string(),
        headers: HashMap::new(),
    };
    
    assert_eq!(config.url, "http://localhost:8080/mcp");
    assert!(config.headers.is_empty());
}

#[test]
fn test_mcp_server_config_url_validation() {
    // Test various URL formats
    let configs = vec![
        "http://localhost:3000/mcp",
        "https://api.example.com/mcp",
        "http://127.0.0.1:8080/mcp",
        "https://mcp-server.company.com/api/v1/mcp",
    ];
    
    for url in configs {
        let config = McpServerConfig {
            url: url.to_string(),
            headers: HashMap::new(),
        };
        assert_eq!(config.url, url);
    }
}

#[test]
fn test_mcp_server_config_headers_manipulation() {
    let mut config = McpServerConfig {
        url: "http://localhost:3000/mcp".to_string(),
        headers: HashMap::new(),
    };
    
    // Add headers
    config.headers.insert("Authorization".to_string(), "Bearer token".to_string());
    config.headers.insert("User-Agent".to_string(), "aether/0.1.0".to_string());
    
    assert_eq!(config.headers.len(), 2);
    
    // Remove a header
    config.headers.remove("User-Agent");
    assert_eq!(config.headers.len(), 1);
    assert!(config.headers.contains_key("Authorization"));
    assert!(!config.headers.contains_key("User-Agent"));
}

// Note: These tests don't actually connect to real MCP servers
// In a real scenario, you would need running MCP servers to test against
// For now, these tests validate the basic structure and configuration