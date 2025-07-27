use aether::mcp::McpClient;
use aether::mcp_config::McpServerConfig;
use anyhow::Result;
use std::collections::HashMap;

#[tokio::test]
async fn test_mcp_client_creation() {
    let _client = McpClient::new();
    // Client no longer has built-in tool registry methods
    // Tools are discovered via discover_tools() and managed in separate ToolRegistry
}

#[tokio::test]
async fn test_mcp_server_config() {
    let config = McpServerConfig::Http {
        url: "http://localhost:3000/mcp".to_string(),
        headers: HashMap::new(),
    };

    match config {
        McpServerConfig::Http { url, headers } => {
            assert_eq!(url, "http://localhost:3000/mcp");
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

    let config = McpServerConfig::Http {
        url: "https://api.example.com/mcp".to_string(),
        headers,
    };

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

    let config = McpServerConfig::Http {
        url: "https://mcp.example.com".to_string(),
        headers,
    };

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
    let client = McpClient::new();

    // Test that tool discovery returns empty list when no servers connected
    let discovered_tools = client.discover_tools().await.unwrap();
    assert!(discovered_tools.is_empty());
}

#[test]
fn test_mcp_server_config_with_empty_headers() {
    let config = McpServerConfig::Http {
        url: "http://localhost:8080/mcp".to_string(),
        headers: HashMap::new(),
    };

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
        let config = McpServerConfig::Http {
            url: url.to_string(),
            headers: HashMap::new(),
        };
        match config {
            McpServerConfig::Http {
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
    let mut config = McpServerConfig::Http {
        url: "http://localhost:3000/mcp".to_string(),
        headers: HashMap::new(),
    };

    // Add headers
    match &mut config {
        McpServerConfig::Http { headers, .. } => {
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
