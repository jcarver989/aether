use acp_utils::client::AcpClientError;
use std::fmt;

#[derive(Debug)]
pub enum WispError {
    Acp(AcpClientError),
    IoError(std::io::Error),
    Other(String),
}

impl fmt::Display for WispError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acp(e) => write!(f, "{e}"),
            Self::IoError(e) => write!(f, "I/O error: {e}"),
            Self::Other(e) => write!(f, "Error: {e}"),
        }
    }
}

impl std::error::Error for WispError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Acp(e) => Some(e),
            Self::IoError(e) => Some(e),
            Self::Other(_) => None,
        }
    }
}

impl From<std::io::Error> for WispError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<AcpClientError> for WispError {
    fn from(e: AcpClientError) -> Self {
        Self::Acp(e)
    }
}
