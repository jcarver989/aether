use super::McpError;
use super::config::ServerConfig;
use super::manager::connect_to_server;
use super::mcp_client::McpClient;
use rmcp::{
    ErrorData, RoleClient, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool as RmcpTool,
    },
    service::{RequestContext, RunningService},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{
    fs::{create_dir_all, remove_dir_all, write},
    task::JoinHandle,
};

/// A tool definition written to disk for agent browsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFileEntry {
    pub name: String,
    pub description: String,
    pub server: String,
    pub parameters: Value,
}

/// Proxy MCP server that lazily discovers tools from nested MCP servers.
///
/// On creation, it connects to each nested server, lists their tools, and writes
/// tool definitions as JSON files to a well-known directory. It then exposes a
/// single `call_tool` tool that routes calls to the appropriate nested server.
pub struct ToolProxyServer {
    /// MCP client connections to nested servers
    clients: HashMap<String, Arc<RunningService<RoleClient, McpClient>>>,
    /// Task handles for server processes (kept alive for lifetime management)
    _server_tasks: Vec<JoinHandle<()>>,
    /// Directory where tool definitions are written
    tool_dir: PathBuf,
    /// The proxy name (used in instructions)
    name: String,
    /// (`server_name`, description) pairs for connected servers
    server_descriptions: Vec<(String, String)>,
}

impl ToolProxyServer {
    /// Connect to all nested MCP servers, discover their tools, and write
    /// tool definition files to disk.
    pub async fn connect(
        name: &str,
        configs: Vec<ServerConfig>,
        create_mcp_client: impl Fn() -> McpClient,
    ) -> Result<Self, McpError> {
        let tool_dir = tool_proxy_dir(name)?;
        if tool_dir.exists() {
            remove_dir_all(&tool_dir)
                .await
                .map_err(|e| McpError::Other(format!("Failed to clean tool-proxy dir: {e}")))?;
        }

        let mut clients = HashMap::new();
        let mut server_tasks = Vec::new();
        let mut server_descriptions = Vec::new();

        for config in configs {
            let server_name = config.name().to_string();
            match connect_to_server(config, &create_mcp_client).await {
                Ok((client, task)) => {
                    if let Err(e) =
                        Self::discover_and_write_tools(&server_name, &client, &tool_dir).await
                    {
                        tracing::warn!(
                            "Failed to discover tools for nested server '{server_name}': {e}"
                        );
                        continue;
                    }

                    let description = extract_server_description(&client, &server_name);
                    server_descriptions.push((server_name.clone(), description));

                    clients.insert(server_name, Arc::new(client));
                    if let Some(handle) = task {
                        server_tasks.push(handle);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to nested server '{server_name}': {e}");
                }
            }
        }

        Ok(Self {
            clients,
            _server_tasks: server_tasks,
            tool_dir,
            name: name.to_string(),
            server_descriptions,
        })
    }

    async fn discover_and_write_tools(
        server_name: &str,
        client: &RunningService<RoleClient, McpClient>,
        tool_dir: &std::path::Path,
    ) -> Result<(), McpError> {
        let tools_response = client.list_tools(None).await.map_err(|e| {
            McpError::ToolDiscoveryFailed(format!(
                "Failed to list tools for nested server '{server_name}': {e}"
            ))
        })?;

        let server_dir = tool_dir.join(server_name);
        create_dir_all(&server_dir).await?;

        for tool in &tools_response.tools {
            let entry = ToolFileEntry {
                name: tool.name.to_string(),
                description: tool.description.clone().unwrap_or_default().to_string(),
                server: server_name.to_string(),
                parameters: Value::Object((*tool.input_schema).clone()),
            };

            let file_path = server_dir.join(format!("{}.json", tool.name));
            let json = serde_json::to_string_pretty(&entry)?;
            write(&file_path, json).await?;
        }

        Ok(())
    }

    fn call_tool_schema() -> Arc<Map<String, Value>> {
        Arc::new(
            json!({
                "type": "object",
                "properties": {
                    "server": {
                        "type": "string",
                        "description": "The server name (directory name in the tool-proxy folder)"
                    },
                    "tool": {
                        "type": "string",
                        "description": "The tool name (file name without .json)"
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Arguments to pass to the tool"
                    }
                },
                "required": ["server", "tool", "arguments"]
            })
            .as_object()
            .unwrap()
            .clone(),
        )
    }
}

impl ServerHandler for ToolProxyServer {
    fn get_info(&self) -> ServerInfo {
        use std::fmt::Write;

        let mut instructions = format!(
            "You are connected to a set of MCP servers, whose tools are available at `{tool_dir}`.\n\
             Each subdirectory in `{tool_dir}` represents a MCP server you're connected. And each subdir contains tool definitions in the form of JSON files.\n\
             Browse or grep the directory to discover tools, then use `call_tool` to execute them.",
            tool_dir = self.tool_dir.display()
        );

        if !self.server_descriptions.is_empty() {
            instructions.push_str("\n\n## Connected Servers\n");
            for (name, desc) in &self.server_descriptions {
                let _ = writeln!(instructions, "- **{name}**: {desc}");
            }
        }

        ServerInfo {
            server_info: Implementation {
                name: format!("tool-proxy-{}", self.name),
                version: "0.1.0".to_string(),
                description: None,
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(instructions),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tool = RmcpTool::new(
            "call_tool",
            "Execute a tool on a nested MCP server. Browse the tool-proxy directory to discover available tools first.",
            Self::call_tool_schema(),
        );

        Ok(ListToolsResult {
            tools: vec![tool],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if request.name.as_ref() != "call_tool" {
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "Unknown tool: {}. This proxy only exposes 'call_tool'.",
                request.name
            ))]));
        }

        let args = request.arguments.unwrap_or_default();

        let server_name = args.get("server").and_then(|v| v.as_str()).ok_or_else(|| {
            ErrorData::invalid_params("Missing required parameter: 'server'", None)
        })?;

        let tool_name = args
            .get("tool")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ErrorData::invalid_params("Missing required parameter: 'tool'", None))?;

        let tool_arguments = args.get("arguments").and_then(|v| v.as_object()).cloned();

        let client = self.clients.get(server_name).ok_or_else(|| {
            ErrorData::invalid_params(format!("Unknown server: '{server_name}'"), None)
        })?;

        let nested_request = CallToolRequestParams {
            name: tool_name.to_string().into(),
            arguments: tool_arguments,
            meta: None,
            task: None,
        };

        client.call_tool(nested_request).await.map_err(|e| {
            ErrorData::internal_error(
                format!("Failed to call tool '{tool_name}' on server '{server_name}': {e}"),
                None,
            )
        })
    }
}

/// Extract a one-line description for a nested server from its peer info.
///
/// Uses `server_info.description`, falling back to the server name.
fn extract_server_description(
    client: &RunningService<RoleClient, McpClient>,
    server_name: &str,
) -> String {
    client
        .peer_info()
        .and_then(|info| {
            info.server_info
                .description
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| server_name.to_string())
}

/// Returns the directory for a tool-proxy's tool definitions.
///
/// Uses `$AETHER_HOME/tool-proxy/<name>` or `~/.aether/tool-proxy/<name>`.
fn tool_proxy_dir(name: &str) -> Result<PathBuf, McpError> {
    let base =
        super::aether_home().ok_or_else(|| McpError::Other("Home directory not set".into()))?;
    Ok(base.join("tool-proxy").join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_file_entry_serialization() {
        let entry = ToolFileEntry {
            name: "create_issue".to_string(),
            description: "Create a GitHub issue".to_string(),
            server: "github".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repo": { "type": "string" },
                    "title": { "type": "string" }
                },
                "required": ["repo", "title"]
            }),
        };

        let json_str = serde_json::to_string_pretty(&entry).unwrap();
        let deserialized: ToolFileEntry = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.name, "create_issue");
        assert_eq!(deserialized.server, "github");
        assert_eq!(deserialized.description, "Create a GitHub issue");
    }

    #[test]
    fn call_tool_schema_is_valid() {
        let schema = ToolProxyServer::call_tool_schema();
        assert_eq!(schema.get("type").unwrap(), "object");

        let properties = schema.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("server"));
        assert!(properties.contains_key("tool"));
        assert!(properties.contains_key("arguments"));

        let required = schema.get("required").unwrap().as_array().unwrap();
        assert_eq!(required.len(), 3);
    }

    #[test]
    fn tool_proxy_dir_appends_correct_suffix() {
        // Test that tool_proxy_dir builds the right path from aether_home().
        // We can't safely mutate env vars in tests, so we verify the suffix
        // logic by checking that the result ends with the expected components.
        let dir = tool_proxy_dir("proxy").unwrap();
        assert!(
            dir.ends_with("tool-proxy/proxy"),
            "Expected path to end with tool-proxy/proxy, got: {}",
            dir.display()
        );
    }

    #[test]
    fn write_and_read_tool_files() {
        let tmp = tempfile::tempdir().unwrap();
        let tool_dir = tmp.path().to_path_buf();
        let server_dir = tool_dir.join("test-server");
        std::fs::create_dir_all(&server_dir).unwrap();

        let entry = ToolFileEntry {
            name: "my_tool".to_string(),
            description: "Does stuff".to_string(),
            server: "test-server".to_string(),
            parameters: json!({"type": "object", "properties": {}}),
        };

        let file_path = server_dir.join("my_tool.json");
        let json = serde_json::to_string_pretty(&entry).unwrap();
        std::fs::write(&file_path, &json).unwrap();

        // Verify we can read it back
        let contents = std::fs::read_to_string(&file_path).unwrap();
        let parsed: ToolFileEntry = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed.name, "my_tool");
        assert_eq!(parsed.server, "test-server");
    }
}
