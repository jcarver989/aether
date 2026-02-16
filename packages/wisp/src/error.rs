use std::fmt;

#[derive(Debug)]
pub enum WispError {
    AgentSpawnFailed(std::io::Error),
    AgentCrashed(String),
    AcpError(agent_client_protocol::Error),
    IoError(std::io::Error),
}

impl fmt::Display for WispError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AgentSpawnFailed(e) => write!(f, "failed to spawn agent subprocess: {e}"),
            Self::AgentCrashed(msg) => write!(f, "agent subprocess crashed: {msg}"),
            Self::AcpError(e) => write!(f, "ACP error: {e}"),
            Self::IoError(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for WispError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AgentSpawnFailed(e) | Self::IoError(e) => Some(e),
            Self::AcpError(e) => Some(e),
            Self::AgentCrashed(_) => None,
        }
    }
}

impl From<std::io::Error> for WispError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<agent_client_protocol::Error> for WispError {
    fn from(e: agent_client_protocol::Error) -> Self {
        Self::AcpError(e)
    }
}
