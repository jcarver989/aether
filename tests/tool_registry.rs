mod utils;

use crate::utils::*;
use aether::tools::Tool;
use serde_json::{Map, json};

#[test]
fn test_tool_registry_creation() {
    let registry = create_test_tool_registry();
    assert_eq!(registry.tool_count(), 0);
    assert!(registry.list_tools().is_empty());
}

#[test]
fn test_register_single_tool() {
    let mut registry = create_test_tool_registry();
    let rmcp_tool = create_test_rmcp_tool("read_file", "Read a file from filesystem");

    registry.register_tool("filesystem".to_string(), rmcp_tool);

    assert_eq!(registry.tool_count(), 1);
    assert_tool_in_registry(&registry, "read_file", "filesystem");
}

#[test]
fn test_register_multiple_tools() {
    let mut registry = create_test_tool_registry();
    let tools = vec![
        create_test_rmcp_tool("read_file", "Read a file"),
        create_test_rmcp_tool("write_file", "Write a file"),
        create_test_rmcp_tool("list_files", "List files in directory"),
    ];

    for tool in tools {
        registry.register_tool("filesystem".to_string(), tool);
    }

    assert_eq!(registry.tool_count(), 3);

    // All tools should map to the same server
    assert_tool_in_registry(&registry, "read_file", "filesystem");
    assert_tool_in_registry(&registry, "write_file", "filesystem");
    assert_tool_in_registry(&registry, "list_files", "filesystem");
}

#[test]
fn test_get_tool_description() {
    let mut registry = create_test_tool_registry();
    let rmcp_tool = create_test_rmcp_tool("git_status", "Get git repository status");

    registry.register_tool("git".to_string(), rmcp_tool);

    let description = registry.get_tool_description("git_status");
    assert!(description.is_some());
    assert_eq!(description.unwrap(), "Get git repository status");

    // Test non-existent tool
    assert!(registry.get_tool_description("nonexistent").is_none());
}

#[test]
fn test_tool_description_retrieval() {
    let mut registry = create_test_tool_registry();
    let rmcp_tool = create_test_rmcp_tool("echo", "Echo text back to user");

    registry.register_tool("shell".to_string(), rmcp_tool);

    assert_eq!(
        registry.get_tool_description("echo"),
        Some("Echo text back to user".to_string())
    );
    assert_eq!(registry.get_tool_description("nonexistent"), None);
}

#[test]
fn test_tool_name_conflicts() {
    let mut registry = create_test_tool_registry();

    // Register same tool name from different servers
    let tool1 = create_test_rmcp_tool("status", "Git status");
    let tool2 = create_test_rmcp_tool("status", "System status");

    registry.register_tool("git".to_string(), tool1);
    registry.register_tool("system".to_string(), tool2);

    // Later registration should overwrite
    assert_eq!(registry.tool_count(), 1);
    assert_tool_in_registry(&registry, "status", "system");

    let description = registry.get_tool_description("status").unwrap();
    assert_eq!(description, "System status");
}

#[test]
fn test_tool_parameters() {
    let mut registry = create_test_tool_registry();
    let rmcp_tool = create_test_rmcp_tool("test_tool", "A test tool");

    registry.register_tool("test_server".to_string(), rmcp_tool);

    let params = registry.get_tool_parameters("test_tool");
    assert!(params.is_some());

    let params = params.unwrap();
    assert!(params.is_object());
}

#[test]
fn test_tool_from_rmcp_tool() {
    let rmcp_tool = create_test_rmcp_tool("convert", "Convert file format");
    let tool = Tool::from(rmcp_tool);

    assert_eq!(tool.description, "Convert file format");
    assert!(tool.parameters.is_object());
}

#[test]
fn test_multiple_servers_with_different_tools() {
    let registry = create_test_tool_registry_with_tools(vec![
        ("filesystem", "read", "Read file"),
        ("filesystem", "write", "Write file"),
        ("git", "commit", "Git commit"),
        ("git", "status", "Git status"),
    ]);

    assert_eq!(registry.tool_count(), 4);

    // Verify tool-to-server mappings
    assert_tool_in_registry(&registry, "read", "filesystem");
    assert_tool_in_registry(&registry, "write", "filesystem");
    assert_tool_in_registry(&registry, "commit", "git");
    assert_tool_in_registry(&registry, "status", "git");
}

#[test]
fn test_get_tool_parameters() {
    let mut registry = create_test_tool_registry();
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
    let registry = create_test_tool_registry();

    assert_eq!(registry.tool_count(), 0);
    assert!(registry.list_tools().is_empty());
    assert!(registry.get_tool_description("anything").is_none());
    assert!(registry.get_server_for_tool("anything").is_none());
    assert!(registry.get_tool_description("anything").is_none());
    assert!(registry.get_tool_parameters("anything").is_none());
    assert!(registry.get_tool_parameters("anything").is_none());
}

#[test]
fn test_tool_with_complex_parameters() {
    let mut properties = Map::new();
    properties.insert(
        "source".to_string(),
        json!({
            "type": "string",
            "description": "Source file path"
        }),
    );
    properties.insert(
        "destination".to_string(),
        json!({
            "type": "string",
            "description": "Destination file path"
        }),
    );
    properties.insert(
        "force".to_string(),
        json!({
            "type": "boolean",
            "description": "Force overwrite if destination exists",
            "default": false
        }),
    );

    let rmcp_tool = create_test_rmcp_tool_with_params(
        "move_file",
        "Move a file to another location",
        properties,
        vec!["source", "destination"],
    );

    let mut registry = create_test_tool_registry();
    registry.register_tool("filesystem".to_string(), rmcp_tool);

    let description = registry.get_tool_description("move_file").unwrap();
    assert_eq!(description, "Move a file to another location");

    let params = registry.get_tool_parameters("move_file").unwrap();
    assert_eq!(params["type"], "object");
    assert_eq!(params["properties"]["source"]["type"], "string");
    assert_eq!(params["properties"]["destination"]["type"], "string");
    assert_eq!(params["properties"]["force"]["type"], "boolean");
    assert_eq!(params["properties"]["force"]["default"], false);
    assert_eq!(params["required"], json!(["source", "destination"]));
}

#[test]
fn test_tool_list_consistency() {
    let registry = create_test_tool_registry_with_tools(vec![
        ("server1", "tool_a", "Tool A"),
        ("server1", "tool_b", "Tool B"),
        ("server1", "tool_c", "Tool C"),
    ]);

    let tool_list = registry.list_tools();
    assert_eq!(tool_list.len(), 3);
    assert_eq!(registry.tool_count(), 3);

    // Verify all tools are present
    for tool_name in &tool_list {
        assert!(registry.get_tool_description(tool_name).is_some());
        assert!(registry.get_server_for_tool(tool_name).is_some());
        assert!(registry.get_tool_description(tool_name).is_some());
    }
}
