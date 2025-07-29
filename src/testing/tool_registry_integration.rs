use crate::testing::InMemoryFileSystem;
use rmcp::{
    RoleClient, RoleServer, ServerHandler, Service,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{CallToolRequestParam, ClientInfo, Implementation, ServerCapabilities, ServerInfo},
    schemars::JsonSchema,
    service::RunningService,
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Test-specific MCP client that uses in-memory transport
pub struct TestMcpClient {
    servers: indexmap::IndexMap<String, RunningService<RoleClient, ClientInfo>>,
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
            servers: indexmap::IndexMap::new(),
            _server_handles: Vec::new(),
        }
    }

    pub async fn connect_test_server<S: Service<RoleServer>>(
        &mut self,
        name: String,
        server: S,
    ) -> Result<(), super::ConnectError> {
        let client_info = ClientInfo {
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "0.1.0".to_string(),
            },
            ..Default::default()
        };

        let (server_handle, client) = super::connect(server, client_info).await?;
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
    pub async fn write_file(&self, request: Parameters<super::WriteFileRequest>) -> String {
        let Parameters(super::WriteFileRequest { path, content }) = request;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolRegistry;

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

        let server1 = super::super::FileServerMcp::new(fs1.clone());
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

        // Verify we have 3 tools total (write_file from file-server is overwritten by multi-server)
        assert_eq!(registry.tool_count(), 3);

        // Verify server mapping - file-server has 1 write_file, multi-server has 3 tools
        // Note: write_file from multi-server will overwrite write_file from file-server
        // since tool names are unique in the registry
        assert_eq!(
            registry.get_server_for_tool("write_file"),
            Some(&"multi-server".to_string())
        );
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
}
