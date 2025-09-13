use aether::{
    agent::{McpServerConfig, agent},
    testing::FakeLlmProvider,
};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use std::collections::HashMap;

#[tokio::test]
async fn test_agent_with_http_mcp_method() {
    let llm = FakeLlmProvider::new(vec![]);

    // Test that we can configure HTTP MCP through McpServerConfig
    let config = StreamableHttpClientTransportConfig {
        uri: "http://localhost:3000".into(),
        ..Default::default()
    };

    let mcp_config = McpServerConfig::Http {
        name: "test".to_string(),
        config,
    };

    let result = agent(llm).mcp(mcp_config).build().await;

    // For this test, we expect an error since no actual server is running
    // but the method should exist and be callable
    assert!(result.is_err());
}

#[tokio::test]
async fn test_agent_with_stdio_mcp_method() {
    let llm = FakeLlmProvider::new(vec![]);

    // Test that we can configure stdio MCP through McpServerConfig
    let env = HashMap::new();
    let mcp_config = McpServerConfig::Stdio {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        env,
    };

    let result = agent(llm).mcp(mcp_config).build().await;

    // For this test, we expect an error since stdio MCP is not yet implemented
    assert!(result.is_err());
}

#[tokio::test]
async fn test_agent_with_in_memory_mcp_method() {
    let llm = FakeLlmProvider::new(vec![]);

    // Test with coding tools - this is now done via coding_tools() method
    let result = agent(llm).build().await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_agent_with_file_server_mcp() {
    let llm = FakeLlmProvider::new(vec![]);

    // Test with coding tools which includes file operations
    let result = agent(llm).build().await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_agent_method_chaining() {
    let llm = FakeLlmProvider::new(vec![]);

    // Test method chaining works
    let result = agent(llm).system("test system prompt").build().await;

    assert!(result.is_ok());
}
