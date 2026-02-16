/// Errors returned by ACP server-side actor operations.
#[derive(Debug, thiserror::Error)]
pub enum AcpServerError {
    #[error("ACP actor stopped")]
    ActorStopped,

    #[error("ACP protocol error: {0}")]
    Protocol(String),
}
