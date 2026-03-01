use super::{McpError, Result, config::ServerConfig, mcp_client::McpClient};
use crate::transport::create_in_memory_transport;
use rmcp::{
    RoleClient, RoleServer, ServiceExt,
    serve_client,
    service::{DynService, RunningService},
    transport::{StreamableHttpClientTransport, TokioChildProcess},
};
use tokio::{process::Command, task::JoinHandle};

/// Connect to an MCP server described by `config`, returning the running client
/// and an optional background task handle (for Stdio child processes / in-memory servers).
///
/// This is the shared connection logic used by `McpManager::add_mcp` and
/// proxied server registration.
pub(crate) async fn connect_to_server(
    config: ServerConfig,
    create_mcp_client: impl Fn() -> McpClient,
) -> Result<(
    RunningService<RoleClient, McpClient>,
    Option<JoinHandle<()>>,
)> {
    match config {
        ServerConfig::Stdio { command, args, .. } => {
            let mut cmd = Command::new(&command);
            cmd.args(&args);
            let mcp_client = create_mcp_client();
            let client = mcp_client.serve(TokioChildProcess::new(cmd)?).await?;
            Ok((client, None))
        }

        ServerConfig::Http { name, config: cfg } => {
            let transport = StreamableHttpClientTransport::from_config(cfg);
            let mcp_client = create_mcp_client();
            let client = serve_client(mcp_client, transport).await.map_err(|e| {
                McpError::ConnectionFailed(format!(
                    "Failed to connect to HTTP server '{name}': {e}"
                ))
            })?;
            Ok((client, None))
        }

        ServerConfig::InMemory { name, server } => {
            let mcp_client = create_mcp_client();
            let (client, handle) = serve_in_memory(server, mcp_client, &name).await?;
            Ok((client, Some(handle)))
        }
    }
}

/// Spawn an in-memory MCP server on a background task and connect a client to it.
///
/// Returns the running client service and the server's join handle.
pub(crate) async fn serve_in_memory(
    server: Box<dyn DynService<RoleServer>>,
    mcp_client: McpClient,
    label: &str,
) -> Result<(RunningService<RoleClient, McpClient>, JoinHandle<()>)> {
    let (client_transport, server_transport) = create_in_memory_transport();

    let server_handle = tokio::spawn(async move {
        match server.serve(server_transport).await {
            Ok(_service) => {
                std::future::pending::<()>().await;
            }
            Err(e) => {
                eprintln!("MCP server error: {e}");
            }
        }
    });

    let client = serve_client(mcp_client, client_transport)
        .await
        .map_err(|e| {
            McpError::ConnectionFailed(format!(
                "Failed to connect to in-memory server '{label}': {e}"
            ))
        })?;

    Ok((client, server_handle))
}
