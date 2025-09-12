mod utils;

use crate::utils::*;
use aether_core::mcp::McpManager;
use std::collections::HashMap;

#[tokio::test]
async fn test_mcp_client_creation() {
    let _client = create_test_mcp_client();
    // Client no longer has built-in tool registry methods
    // Tools are discovered via discover_tools() and managed in separate ToolRegistry
}

#[tokio::test]
async fn test_mcp_client_with_http_server() {
    let mut client = McpManager::new();
    let server_name = "test_server".to_string();
    let url = TEST_SERVER_URL.to_string();
    let headers = HashMap::new();

    // This would fail in a real test since there's no server, but it tests the API
    let result = client.with_http_mcp(server_name.clone(), url.clone(), headers.clone()).await;
    
    // The connection will fail, but we can still test that the API exists
    // In a real test environment, this would succeed
    assert!(result.is_err()); // Expected since no real server is running
}

#[tokio::test]
async fn test_mcp_client_with_headers() {
    let mut client = McpManager::new();
    let server_name = "test_server_with_headers".to_string();
    let url = "https://api.example.com/mcp".to_string();
    
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    // Test that the API accepts headers
    let result = client.with_http_mcp(server_name, url, headers).await;
    
    // The connection will fail, but we can test that the API accepts headers
    assert!(result.is_err()); // Expected since no real server is running
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

#[tokio::test]
async fn test_mcp_client_with_coding_server() {
    let mut client = McpManager::new();
    
    // Test adding coding server
    client.add_coding_server("coding".to_string()).await.unwrap();
    
    // Test tool discovery
    client.discover_tools().await.unwrap();
    
    // Should have tools from the coding server
    let tool_definitions = client.get_tool_definitions();
    assert!(!tool_definitions.is_empty());
}

#[tokio::test]
async fn test_mcp_manager_builder_pattern() {
    let mut manager = McpManager::new();
    
    // Test that we can add a coding server
    manager.add_coding_server("coding".to_string()).await.unwrap();
    
    // Test that we can discover tools
    manager.discover_tools().await.unwrap();
    
    // Verify tools were discovered
    let tools = manager.get_tool_definitions();
    assert!(!tools.is_empty());
    
    // Test that all tools are properly namespaced
    for tool in &tools {
        assert!(tool.name.contains("::"));
        assert!(tool.server.is_some());
    }
}

// Note: These tests don't actually connect to real MCP servers
// In a real scenario, you would need running MCP servers to test against
// For now, these tests validate the basic structure and configuration
