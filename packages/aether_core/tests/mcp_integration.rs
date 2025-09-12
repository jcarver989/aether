mod utils;

use crate::utils::*;
use aether_core::mcp::mcp_config::McpServerConfig;
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
        McpServerConfig::Http { name, url, headers } => {
            assert_eq!(name, "test_server");
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
        McpServerConfig::Http { name, url, headers } => {
            assert_eq!(name, "test_server_with_headers");
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
async fn test_mcp_client_tool_discovery() {
    let mut client = create_test_mcp_client();

    // Test that tool discovery succeeds when no servers connected
    client.discover_tools().await.unwrap();

    // Test that tool definitions are empty when no servers connected
    let tool_definitions = client.get_tool_definitions();
    assert!(tool_definitions.is_empty());
}

#[test]
fn test_mcp_server_config_with_empty_headers() {
    let config = create_test_mcp_server_config("http://localhost:8080/mcp");

    match config {
        McpServerConfig::Http {
            name, url, headers, ..
        } => {
            assert_eq!(name, "test_server");
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
            aether_core::mcp::mcp_config::McpServerConfig::Http {
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
        aether_core::mcp::mcp_config::McpServerConfig::Http { headers, .. } => {
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
