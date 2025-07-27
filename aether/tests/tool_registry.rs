use aether::mcp::registry::{ToolRegistry, Tool};
use rmcp::model::Tool as RmcpTool;
use serde_json::{json, Map};
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

#[test]
fn test_get_tool_parameters() {
    let mut registry = ToolRegistry::new();
    let rmcp_tool = create_test_rmcp_tool("copy_file", "Copy a file to another location");
    
    registry.register_tool("filesystem".to_string(), rmcp_tool);
    
    let params = registry.get_tool_parameters("copy_file");
    assert!(params.is_some());
    
    let params = params.unwrap();
    assert_eq!(params["type"], "object");
    assert!(params["properties"].is_object());
    assert!(params["required"].is_array());
    
    // Test non-existent tool
    assert!(registry.get_tool_parameters("nonexistent").is_none());
}

#[test]
fn test_tool_registry_empty_operations() {
    let registry = ToolRegistry::new();
    
    assert_eq!(registry.tool_count(), 0);
    assert!(registry.list_tools().is_empty());
    assert!(registry.get_tool("anything").is_none());
    assert!(registry.get_server_for_tool("anything").is_none());
    assert!(registry.get_tool_description("anything").is_none());
    assert!(registry.get_tool_parameters("anything").is_none());
    assert!(registry.as_json_schemas().is_empty());
}

#[test]
fn test_tool_with_complex_parameters() {
    let mut properties = Map::new();
    properties.insert("source".to_string(), json!({
        "type": "string",
        "description": "Source file path"
    }));
    properties.insert("destination".to_string(), json!({
        "type": "string", 
        "description": "Destination file path"
    }));
    properties.insert("force".to_string(), json!({
        "type": "boolean",
        "description": "Force overwrite if destination exists",
        "default": false
    }));
    
    let mut schema = Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), json!(properties));
    schema.insert("required".to_string(), json!(["source", "destination"]));
    
    let rmcp_tool = RmcpTool::new(
        "move_file".to_string(), 
        "Move a file to another location".to_string(), 
        Arc::new(schema)
    );
    
    let mut registry = ToolRegistry::new();
    registry.register_tool("filesystem".to_string(), rmcp_tool);
    
    let tool = registry.get_tool("move_file").unwrap();
    assert_eq!(tool.name, "move_file");
    assert_eq!(tool.description, "Move a file to another location");
    
    let params = &tool.parameters;
    assert_eq!(params["type"], "object");
    assert_eq!(params["properties"]["source"]["type"], "string");
    assert_eq!(params["properties"]["destination"]["type"], "string");
    assert_eq!(params["properties"]["force"]["type"], "boolean");
    assert_eq!(params["properties"]["force"]["default"], false);
    assert_eq!(params["required"], json!(["source", "destination"]));
}

#[test]
fn test_tool_list_consistency() {
    let mut registry = ToolRegistry::new();
    
    // Add some tools
    let tools = vec![
        create_test_rmcp_tool("tool_a", "Tool A"),
        create_test_rmcp_tool("tool_b", "Tool B"),
        create_test_rmcp_tool("tool_c", "Tool C"),
    ];
    registry.register_tools("server1".to_string(), tools);
    
    let tool_list = registry.list_tools();
    assert_eq!(tool_list.len(), 3);
    assert_eq!(registry.tool_count(), 3);
    
    // Verify all tools are present
    for tool_name in &tool_list {
        assert!(registry.get_tool(tool_name).is_some());
        assert!(registry.get_server_for_tool(tool_name).is_some());
        assert!(registry.get_tool_description(tool_name).is_some());
    }
}