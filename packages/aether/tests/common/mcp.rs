#![allow(dead_code)]

use aether::fs::Fs;
use aether::{testing::InMemoryFileSystem, transport::create_in_memory_transport};
use rmcp::{
    RoleClient, RoleServer, ServerHandler, Service,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ClientInfo, Implementation, ServerCapabilities, ServerInfo},
    serve_client, serve_server,
    service::{ClientInitializeError, RunningService, ServerInitializeError},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
}

/// A real MCP server that provides file writing functionality
#[derive(Debug, Clone)]
pub struct FileServerMcp {
    filesystem: Arc<InMemoryFileSystem>,
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FileServerMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "file-server-mcp".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: Some("A file server with write capabilities".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool_router]
impl FileServerMcp {
    pub fn new(filesystem: InMemoryFileSystem) -> Self {
        Self {
            filesystem: Arc::new(filesystem),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Write content to a file in the in-memory filesystem")]
    pub async fn write_file(&self, request: Parameters<WriteFileRequest>) -> String {
        let Parameters(WriteFileRequest { path, content }) = request;

        match self.filesystem.write_file(&path, &content).await {
            Ok(_) => format!("Successfully wrote {} bytes to {}", content.len(), path),
            Err(e) => format!("Error writing file: {e}"),
        }
    }
}

/// Helper function to connect an MCP server and client via in-memory transport
/// This handles the initialization handshake by running both concurrently
pub async fn connect<S>(
    server: S,
    client_info: ClientInfo,
) -> Result<
    (
        RunningService<RoleServer, S>,
        RunningService<RoleClient, ClientInfo>,
    ),
    ConnectError,
>
where
    S: Service<RoleServer>,
{
    let (client_transport, server_transport) = create_in_memory_transport();

    let (server_result, client_result) = tokio::join!(
        serve_server(server, server_transport),
        serve_client(client_info, client_transport)
    );

    let server = server_result.map_err(ConnectError::ServerInit)?;
    let client = client_result.map_err(ConnectError::ClientInit)?;

    Ok((server, client))
}

#[derive(Debug)]
pub enum ConnectError {
    ServerInit(ServerInitializeError),
    ClientInit(ClientInitializeError),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::ServerInit(e) => write!(f, "Server initialization failed: {e}"),
            ConnectError::ClientInit(e) => write!(f, "Client initialization failed: {e}"),
        }
    }
}

impl std::error::Error for ConnectError {}
