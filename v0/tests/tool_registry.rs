use aether::mcp::registry::{ToolRegistry, Tool};
use rmcp::model::Tool as RmcpTool;
use serde_json::{json, Map, Value};
use std::sync::Arc;

fn create_test_rmcp_tool(name: &str, description: &str) -> RmcpTool {
    let mut properties = Map::new();
    properties.insert("path".to_string(), json!({"type": "string", "description": "File path"}));
    
    let mut schema = Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), json!(properties));
    schema.insert("required".to_string(), json!(["path"]));

    RmcpTool::new(name.to_string(), description.to_string(), Arc::new(schema))
}

#[test]
fn test_tool_registry_creation() {
    let registry = ToolRegistry::new();
    assert_eq!(registry.tool_count(), 0);
    assert!(registry.list_tools().is_empty());
}

#[test]
fn test_register_single_tool() {
    let mut registry = ToolRegistry::new();
    let rmcp_tool = create_test_rmcp_tool("read_file", "Read a file from filesystem");
    
    registry.register_tool("filesystem".to_string(), rmcp_tool);
    
    assert_eq!(registry.tool_count(), 1);
    assert!(registry.list_tools().contains(&"read_file".to_string()));
    assert_eq!(registry.get_server_for_tool("read_file"), Some(&"filesystem".to_string()));
}

#[test]
fn test_register_multiple_tools() {
    let mut registry = ToolRegistry::new();
    let tools = vec![
        create_test_rmcp_tool("read_file", "Read a file"),
        create_test_rmcp_tool("write_file", "Write a file"),
        create_test_rmcp_tool("list_files", "List files in directory"),
    ];
    
    registry.register_tools("filesystem".to_string(), tools);
    
    assert_eq!(registry.tool_count(), 3);
    let tool_list = registry.list_tools();
    assert!(tool_list.contains(&"read_file".to_string()));
    assert!(tool_list.contains(&"write_file".to_string()));
    assert!(tool_list.contains(&"list_files".to_string()));
    
    // All tools should map to the same server
    assert_eq!(registry.get_server_for_tool("read_file"), Some(&"filesystem".to_string()));
    assert_eq!(registry.get_server_for_tool("write_file"), Some(&"filesystem".to_string()));
    assert_eq!(registry.get_server_for_tool("list_files"), Some(&"filesystem".to_string()));
}

#[test]
fn test_get_tool() {
    let mut registry = ToolRegistry::new();
    let rmcp_tool = create_test_rmcp_tool("git_status", "Get git repository status");
    
    registry.register_tool("git".to_string(), rmcp_tool);
    
    let tool = registry.get_tool("git_status");
    assert!(tool.is_some());
    
    let tool = tool.unwrap();
    assert_eq!(tool.name, "git_status");
    assert_eq!(tool.description, "Get git repository status");
    assert_eq!(tool.server_name, "git");
    
    // Test non-existent tool
    assert!(registry.get_tool("nonexistent").is_none());
}

#[test]
fn test_get_tool_description() {
    let mut registry = ToolRegistry::new();
    let rmcp_tool = create_test_rmcp_tool("echo", "Echo text back to user");
    
    registry.register_tool("shell".to_string(), rmcp_tool);
    
    assert_eq!(registry.get_tool_description("echo"), Some("Echo text back to user".to_string()));
    assert_eq!(registry.get_tool_description("nonexistent"), None);
}

#[test]
fn test_tool_name_conflicts() {
    let mut registry = ToolRegistry::new();
    
    // Register same tool name from different servers
    let tool1 = create_test_rmcp_tool("status", "Git status");
    let tool2 = create_test_rmcp_tool("status", "System status");
    
    registry.register_tool("git".to_string(), tool1);
    registry.register_tool("system".to_string(), tool2);
    
    // Later registration should overwrite
    assert_eq!(registry.tool_count(), 1);
    assert_eq!(registry.get_server_for_tool("status"), Some(&"system".to_string()));
    
    let tool = registry.get_tool("status").unwrap();
    assert_eq!(tool.description, "System status");
    assert_eq!(tool.server_name, "system");
}

#[test]
fn test_as_json_schemas() {
    let mut registry = ToolRegistry::new();
    let rmcp_tool = create_test_rmcp_tool("test_tool", "A test tool");
    
    registry.register_tool("test_server".to_string(), rmcp_tool);
    
    let schemas = registry.as_json_schemas();
    assert_eq!(schemas.len(), 1);
    
    let schema = &schemas[0];
    assert_eq!(schema["type"], "function");
    assert_eq!(schema["function"]["name"], "test_tool");
    assert_eq!(schema["function"]["description"], "A test tool");
    assert!(schema["function"]["parameters"].is_object());
}

#[test]
fn test_tool_from_rmcp_tool() {
    let rmcp_tool = create_test_rmcp_tool("convert", "Convert file format");
    let tool = Tool::from_rmcp_tool("converter".to_string(), rmcp_tool);
    
    assert_eq!(tool.name, "convert");
    assert_eq!(tool.description, "Convert file format");
    assert_eq!(tool.server_name, "converter");
    assert!(tool.parameters.is_object());
}

#[test]
fn test_multiple_servers_with_different_tools() {
    let mut registry = ToolRegistry::new();
    
    // Register tools from filesystem server
    let fs_tools = vec![
        create_test_rmcp_tool("read", "Read file"),
        create_test_rmcp_tool("write", "Write file"),
    ];
    registry.register_tools("filesystem".to_string(), fs_tools);
    
    // Register tools from git server
    let git_tools = vec![
        create_test_rmcp_tool("commit", "Git commit"),
        create_test_rmcp_tool("status", "Git status"),
    ];
    registry.register_tools("git".to_string(), git_tools);
    
    assert_eq!(registry.tool_count(), 4);
    
    // Verify tool-to-server mappings
    assert_eq!(registry.get_server_for_tool("read"), Some(&"filesystem".to_string()));
    assert_eq!(registry.get_server_for_tool("write"), Some(&"filesystem".to_string()));
    assert_eq!(registry.get_server_for_tool("commit"), Some(&"git".to_string()));
    assert_eq!(registry.get_server_for_tool("status"), Some(&"git".to_string()));
}