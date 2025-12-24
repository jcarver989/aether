use std::fmt;

#[derive(Debug)]
pub enum OAuthError {
    AuthorizationRequired { server_id: String },
    UserCancelled,
    TokenExchange(String),
    CredentialStore(String),
    Rmcp(String),
    Io(std::io::Error),
    SerdeJson(serde_json::Error),
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorizationRequired { server_id } => {
                write!(f, "OAuth authorization required for {server_id}")
            }
            Self::UserCancelled => write!(f, "User cancelled authorization"),
            Self::TokenExchange(msg) => write!(f, "Token exchange failed: {msg}"),
            Self::CredentialStore(msg) => write!(f, "Credential storage error: {msg}"),
            Self::Rmcp(msg) => write!(f, "rmcp auth error: {msg}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::SerdeJson(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for OAuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::SerdeJson(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for OAuthError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for OAuthError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerdeJson(e)
    }
}
