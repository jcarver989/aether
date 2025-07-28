mod utils;

use crate::utils::*;
use aether::mcp_config::McpServerConfig;
use color_eyre::Result;
use std::collections::HashMap;

#[tokio::test]
async fn test_mcp_client_creation() {
    let _client = create_test_mcp_client();
    // Client no longer has built-in tool registry methods
    // Tools are discovered via discover_tools() and managed in separate ToolRegistry
}

#[tokio::test]
async fn test_mcp_server_config() {
    let config = create_test_mcp_server_config(TEST_SERVER_URL);

    match config {
        McpServerConfig::Http { url, headers } => {
            assert_eq!(url, TEST_SERVER_URL);
            assert!(headers.is_empty());
        }
        _ => panic!("Expected Http config"),
    }
}

#[tokio::test]
async fn test_mcp_server_config_with_headers() {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let config = create_test_mcp_server_config_with_headers("https://api.example.com/mcp", headers);

    match config {
        McpServerConfig::Http { url, headers } => {
            assert_eq!(url, "https://api.example.com/mcp");
            assert_eq!(headers.len(), 2);
            assert_eq!(
                headers.get("Authorization"),
                Some(&"Bearer token123".to_string())
            );
            assert_eq!(
                headers.get("Content-Type"),
                Some(&"application/json".to_string())
            );
        }
        _ => panic!("Expected Http config"),
    }
}

#[tokio::test]
async fn test_mcp_server_config_serialization() -> Result<()> {
    let mut headers = HashMap::new();
    headers.insert("X-API-Key".to_string(), "secret123".to_string());

    let config = create_test_mcp_server_config_with_headers("https://mcp.example.com", headers);

    let serialized = serde_json::to_string(&config)?;
    let deserialized: McpServerConfig = serde_json::from_str(&serialized)?;

    match deserialized {
        McpServerConfig::Http { url, headers } => {
            assert_eq!(url, "https://mcp.example.com");
            assert_eq!(headers.len(), 1);
            assert_eq!(headers.get("X-API-Key"), Some(&"secret123".to_string()));
        }
        _ => panic!("Expected Http config"),
    }

    Ok(())
}

#[tokio::test]
async fn test_mcp_client_tool_discovery() {
    let client = create_test_mcp_client();

    // Test that tool discovery returns empty list when no servers connected
    let discovered_tools = client.discover_tools().await.unwrap();
    assert!(discovered_tools.is_empty());
}

#[test]
fn test_mcp_server_config_with_empty_headers() {
    let config = create_test_mcp_server_config("http://localhost:8080/mcp");

    match config {
        McpServerConfig::Http { url, headers } => {
            assert_eq!(url, "http://localhost:8080/mcp");
            assert!(headers.is_empty());
        }
        _ => panic!("Expected Http config"),
    }
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
        let config = create_test_mcp_server_config(url);
        match config {
            aether::mcp_config::McpServerConfig::Http {
                url: config_url, ..
            } => {
                assert_eq!(config_url, url);
            }
            _ => panic!("Expected Http config"),
        }
    }
}

#[test]
fn test_mcp_server_config_headers_manipulation() {
    let mut config = create_test_mcp_server_config(TEST_SERVER_URL);

    // Add headers
    match &mut config {
        aether::mcp_config::McpServerConfig::Http { headers, .. } => {
            headers.insert("Authorization".to_string(), "Bearer token".to_string());
            headers.insert("User-Agent".to_string(), "aether/0.1.0".to_string());

            assert_eq!(headers.len(), 2);

            // Remove a header
            headers.remove("User-Agent");
            assert_eq!(headers.len(), 1);
            assert!(headers.contains_key("Authorization"));
            assert!(!headers.contains_key("User-Agent"));
        }
        _ => panic!("Expected Http config"),
    }
}

// Note: These tests don't actually connect to real MCP servers
// In a real scenario, you would need running MCP servers to test against
// For now, these tests validate the basic structure and configuration
