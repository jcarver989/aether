use acp_utils::client::AcpClientError;
use std::fmt;

/// Application-level errors for the Wisp TUI.
/// Used by the MVC controller and view operations.
#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Prompt(AcpClientError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Prompt(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Prompt(e) => Some(e),
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
        Self::Prompt(e)
    }
}

#[derive(Debug)]
pub enum WispError {
    Acp(AcpClientError),
    IoError(std::io::Error),
}

impl fmt::Display for WispError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acp(e) => write!(f, "{e}"),
            Self::IoError(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for WispError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Acp(e) => Some(e),
            Self::IoError(e) => Some(e),
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
