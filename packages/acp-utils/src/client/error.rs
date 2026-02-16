/// Errors returned by ACP client-side operations.
#[derive(Debug, thiserror::Error)]
pub enum AcpClientError {
    #[error("failed to spawn agent subprocess: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("agent subprocess crashed: {0}")]
    AgentCrashed(String),

    #[error("ACP protocol error: {0}")]
    Protocol(#[source] agent_client_protocol::Error),
}

impl From<agent_client_protocol::Error> for AcpClientError {
    fn from(e: agent_client_protocol::Error) -> Self {
        Self::Protocol(e)
    }
}
