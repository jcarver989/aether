mod utils;

use crate::utils::*;
use aether_core::tools::{Tool, ToolRegistry};
use aether_core::testing::{InMemoryFileSystem, connect, ConnectError, FileServerMcp};
use rmcp::{
    RoleClient, RoleServer, ServerHandler, Service,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{CallToolRequestParam, ClientInfo, Implementation, ServerCapabilities, ServerInfo},
    service::RunningService,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, json};
use std::{collections::HashMap, sync::Arc};

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

// Integration test utilities and types

/// Test-specific MCP client that uses in-memory transport
pub struct TestMcpClient {
    servers: HashMap<String, RunningService<RoleClient, ClientInfo>>,
    // Keep server handles alive to prevent transport from closing
    _server_handles: Vec<Box<dyn std::any::Any + Send>>,
}

impl Default for TestMcpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TestMcpClient {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            _server_handles: Vec::new(),
        }
    }

    pub async fn connect_test_server<S: Service<RoleServer>>(
        &mut self,
        name: String,
        server: S,
    ) -> Result<(), ConnectError> {
        let client_info = ClientInfo {
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "0.1.0".to_string(),
            },
            ..Default::default()
        };

        let (server_handle, client) = connect(server, client_info).await?;
        self.servers.insert(name, client);
        self._server_handles.push(Box::new(server_handle));
        Ok(())
    }

    pub async fn discover_tools(&self) -> Result<Vec<(String, rmcp::model::Tool)>, String> {
        let mut discovered_tools = Vec::new();

        for (server_name, client) in &self.servers {
            match client.list_tools(None).await {
                Ok(tools_response) => {
                    for tool in tools_response.tools {
                        discovered_tools.push((server_name.clone(), tool));
                    }
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to discover tools from server {server_name}: {e}"
                    ));
                }
            }
        }

        Ok(discovered_tools)
    }

    pub async fn execute_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let client = self
            .servers
            .get(server_name)
            .ok_or_else(|| format!("Server not found: {server_name}"))?;

        let arguments = args.as_object().cloned();
        let request = CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments,
        };

        let result = client
            .call_tool(request)
            .await
            .map_err(|e| format!("Failed to execute tool: {e}"))?;

        if result.is_error.unwrap_or(false) {
            return Err("Tool execution failed".to_string());
        }

        // Convert the result content to a JSON value
        let result_value = result
            .content
            .first()
            .and_then(|content| content.as_text())
            .map(|text| serde_json::Value::String(text.text.clone()))
            .unwrap_or_else(|| serde_json::Value::String("No result".to_string()));

        Ok(result_value)
    }
}

/// Multi-tool test server
#[derive(Debug, Clone)]
pub struct MultiToolServer {
    filesystem: Arc<InMemoryFileSystem>,
    tool_router: ToolRouter<Self>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ReadFileRequest {
    pub path: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ListFilesRequest {
    pub prefix: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for MultiToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "multi-tool-server".to_string(),
                version: "0.1.0".to_string(),
            },
            instructions: Some("A server with multiple file tools".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool_router]
impl MultiToolServer {
    pub fn new(filesystem: InMemoryFileSystem) -> Self {
        Self {
            filesystem: Arc::new(filesystem),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Write content to a file")]
    pub async fn write_file(&self, request: Parameters<WriteFileRequest>) -> String {
        let Parameters(WriteFileRequest { path, content }) = request;

        match self.filesystem.write_file(&path, &content).await {
            Ok(_) => format!("Successfully wrote {} bytes to {}", content.len(), path),
            Err(e) => format!("Error writing file: {e}"),
        }
    }

    #[tool(description = "Read content from a file")]
    pub async fn read_file(&self, request: Parameters<ReadFileRequest>) -> String {
        let Parameters(ReadFileRequest { path }) = request;

        match self.filesystem.read_file(&path).await {
            Ok(content) => content,
            Err(e) => format!("Error reading file: {e}"),
        }
    }

    #[tool(description = "List files in the filesystem")]
    pub async fn list_files(&self, request: Parameters<ListFilesRequest>) -> String {
        let Parameters(ListFilesRequest { prefix }) = request;

        match self.filesystem.list_files().await {
            Ok(files) => {
                let filtered: Vec<_> = match prefix {
                    Some(p) => files.into_iter().filter(|f| f.starts_with(&p)).collect(),
                    None => files,
                };
                filtered.join("\n")
            }
            Err(e) => format!("Error listing files: {e}"),
        }
    }
}

// Integration tests

#[tokio::test]
async fn test_tool_registry_with_real_mcp_server() {
    // Create a tool registry
    let mut registry = ToolRegistry::new();

    // Create filesystem and server
    let filesystem = InMemoryFileSystem::new();
    let server = MultiToolServer::new(filesystem.clone());

    // Create test MCP client
    let mut test_client = TestMcpClient::new();
    test_client
        .connect_test_server("test-server".to_string(), server)
        .await
        .expect("Failed to connect test server");

    // Discover tools and register them
    let tools = test_client
        .discover_tools()
        .await
        .expect("Failed to discover tools");

    // Verify we discovered 3 tools
    assert_eq!(tools.len(), 3);

    // Register all discovered tools
    for (server_name, tool) in tools {
        registry.register_tool(server_name, tool);
    }

    // Verify registry state
    assert_eq!(registry.tool_count(), 3);

    let tool_list = registry.list_tools();
    assert!(tool_list.contains(&"write_file".to_string()));
    assert!(tool_list.contains(&"read_file".to_string()));
    assert!(tool_list.contains(&"list_files".to_string()));

    // Verify tool descriptions
    assert!(
        registry
            .get_tool_description("write_file")
            .unwrap()
            .contains("Write content to a file")
    );
    assert!(
        registry
            .get_tool_description("read_file")
            .unwrap()
            .contains("Read content from a file")
    );
    assert!(
        registry
            .get_tool_description("list_files")
            .unwrap()
            .contains("List files in the filesystem")
    );

    // Verify server mapping
    assert_eq!(
        registry.get_server_for_tool("write_file"),
        Some(&"test-server".to_string())
    );
    assert_eq!(
        registry.get_server_for_tool("read_file"),
        Some(&"test-server".to_string())
    );
    assert_eq!(
        registry.get_server_for_tool("list_files"),
        Some(&"test-server".to_string())
    );

    // For now, we'll test registry functionality without invoke_tool
    // since it requires a real McpClient instance
}

#[tokio::test]
async fn test_tool_registry_multiple_servers() {
    let mut registry = ToolRegistry::new();

    // Create two different servers
    let fs1 = InMemoryFileSystem::new();
    let fs2 = InMemoryFileSystem::new();

    let server1 = FileServerMcp::new(fs1.clone());
    let server2 = MultiToolServer::new(fs2.clone());

    // Connect both servers
    let mut test_client = TestMcpClient::new();
    test_client
        .connect_test_server("file-server".to_string(), server1)
        .await
        .expect("Failed to connect file server");
    test_client
        .connect_test_server("multi-server".to_string(), server2)
        .await
        .expect("Failed to connect multi server");

    // Discover and register tools from both servers
    let tools = test_client
        .discover_tools()
        .await
        .expect("Failed to discover tools");

    for (server_name, tool) in tools {
        registry.register_tool(server_name, tool);
    }

    // Verify we have 3 tools total (write_file from one server may overwrite the other)
    assert_eq!(registry.tool_count(), 3);

    // Both servers have write_file, so whichever was registered last wins
    // The important thing is that all 3 tools are present
    assert!(registry.get_server_for_tool("write_file").is_some());
    assert_eq!(
        registry.get_server_for_tool("read_file"),
        Some(&"multi-server".to_string())
    );
    assert_eq!(
        registry.get_server_for_tool("list_files"),
        Some(&"multi-server".to_string())
    );

    // For now, we'll test registry functionality without invoke_tool
    // since it requires a real McpClient instance
}

#[tokio::test]
async fn test_tool_registry_parameter_validation() {
    let mut registry = ToolRegistry::new();

    // Create and connect a server
    let fs = InMemoryFileSystem::new();
    let server = MultiToolServer::new(fs);

    let mut test_client = TestMcpClient::new();
    test_client
        .connect_test_server("test-server".to_string(), server)
        .await
        .expect("Failed to connect server");

    // Discover and register tools
    let tools = test_client.discover_tools().await.unwrap();
    for (server_name, tool) in tools {
        registry.register_tool(server_name, tool);
    }

    // Get tool parameters
    let write_params = registry.get_tool_parameters("write_file").unwrap();
    assert!(write_params.is_object());

    let read_params = registry.get_tool_parameters("read_file").unwrap();
    assert!(read_params.is_object());

    let list_params = registry.get_tool_parameters("list_files").unwrap();
    assert!(list_params.is_object());

    // Verify parameter schemas contain expected fields
    let write_schema = write_params.as_object().unwrap();
    assert!(write_schema.contains_key("properties"));

    let props = write_schema.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("path"));
    assert!(props.contains_key("content"));
}
