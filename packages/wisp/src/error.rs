use acp_utils::client::AcpClientError;
use std::fmt;

#[doc = include_str!("docs/app_error.md")]
#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Acp(AcpClientError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Acp(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Acp(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<AcpClientError> for AppError {
    fn from(e: AcpClientError) -> Self {
        Self::Acp(e)
    }
}
