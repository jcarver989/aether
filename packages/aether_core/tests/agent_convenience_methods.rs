use aether_core::{agent::Agent, llm::MockLlm, mcp::builtin_servers::CodingMcp, testing::FileServerMcp, testing::InMemoryFileSystem};
use std::collections::HashMap;

#[tokio::test]
async fn test_agent_with_http_mcp_method() {
    let llm = MockLlm::new();
    let agent = Agent::new(llm, None);
    
    // Test that the method exists and returns a Result
    let headers = HashMap::new();
    let result = agent.with_http_mcp(
        "test".to_string(),
        "http://localhost:3000".to_string(),
        headers,
    ).await;
    
    // For this test, we expect an error since no actual server is running
    // but the method should exist and be callable
    assert!(result.is_err());
}

#[tokio::test]
async fn test_agent_with_stdio_mcp_method() {
    let llm = MockLlm::new();
    let agent = Agent::new(llm, None);
    
    // Test that the method exists and returns a Result
    let env = HashMap::new();
    let result = agent.with_stdio_mcp(
        "test".to_string(),
        "echo".to_string(),
        vec!["hello".to_string()],
        env,
    ).await;
    
    // For this test, we expect an error since stdio MCP is not yet implemented
    assert!(result.is_err());
}

#[tokio::test]
async fn test_agent_with_in_memory_mcp_method() {
    let llm = MockLlm::new();
    let agent = Agent::new(llm, None);
    
    // Test with CodingMcp server
    let result = agent.with_in_memory_mcp(
        "coding".to_string(),
        CodingMcp::new(),
    ).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_agent_with_file_server_mcp() {
    let llm = MockLlm::new();
    let agent = Agent::new(llm, None);
    
    let filesystem = InMemoryFileSystem::new();
    let file_server = FileServerMcp::new(filesystem);
    
    // Test with FileServerMcp
    let result = agent.with_in_memory_mcp(
        "file_server".to_string(),
        file_server,
    ).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_agent_method_chaining() {
    let llm = MockLlm::new();
    let agent = Agent::new(llm, None);
    
    // Test method chaining works
    let filesystem = InMemoryFileSystem::new();
    let file_server = FileServerMcp::new(filesystem);
    
    let result = agent
        .with_coding_tools()
        .await
        .unwrap()
        .with_in_memory_mcp("file_server".to_string(), file_server)
        .await;
    
    assert!(result.is_ok());
}