use super::super::common::*;
use aether::mcp::{ElicitationRequest, McpManager, McpServerConfig};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_mcp_client_creation() {
    let _client = create_test_mcp_client();
    // Client no longer has built-in tool registry methods
    // Tools are discovered via discover_tools() and managed in separate ToolRegistry
}

#[tokio::test]
async fn test_mcp_client_with_http_server() {
    let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(50);
    let mut client = McpManager::new(elicitation_tx, None);
    let server_name = "test_server".to_string();
    let url = TEST_SERVER_URL.to_string();

    // This would fail in a real test since there's no server, but it tests the API
    let config = StreamableHttpClientTransportConfig {
        uri: url.clone().into(),
        ..Default::default()
    };
    let mcp_config = McpServerConfig::Http {
        name: server_name,
        config,
    };
    let result = client.add_mcp(mcp_config).await;

    // The connection will fail, but we can still test that the API exists
    // In a real test environment, this would succeed
    assert!(result.is_err()); // Expected since no real server is running
}

#[tokio::test]
async fn test_mcp_client_with_headers() {
    let (elicitation_tx, _elicitation_rx) = mpsc::channel::<ElicitationRequest>(50);
    let mut client = McpManager::new(elicitation_tx, None);
    let server_name = "test_server_with_headers".to_string();
    let url = "https://api.example.com/mcp".to_string();

    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    // Test that the API accepts headers - encode as auth header for testing
    let auth_header = headers.get("Authorization").cloned();
    let config = StreamableHttpClientTransportConfig {
        uri: url.into(),
        auth_header,
        ..Default::default()
    };
    let mcp_config = McpServerConfig::Http {
        name: server_name,
        config,
    };
    let result = client.add_mcp(mcp_config).await;

    // The connection will fail, but we can test that the API accepts headers
    assert!(result.is_err()); // Expected since no real server is running
}
