use crate::transport::create_in_memory_transport;
use rmcp::{
    RoleClient, RoleServer, Service,
    model::ClientInfo,
    serve_client, serve_server,
    service::{ClientInitializeError, RunningService, ServerInitializeError},
};

/// Helper function to connect an MCP server and client via in-memory transport
/// This handles the initialization handshake by running both concurrently
pub async fn connect<S>(
    server: S,
    client_info: ClientInfo,
) -> Result<(RunningService<RoleServer, S>, RunningService<RoleClient, ClientInfo>), ConnectError>
where
    S: Service<RoleServer>,
{
    let (client_transport, server_transport) = create_in_memory_transport();

    let (server_result, client_result) =
        tokio::join!(serve_server(server, server_transport), serve_client(client_info, client_transport));

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
