use std::fmt;

pub mod anthropic;
pub mod store;

pub use anthropic::{
    AnthropicAuthMode, AuthorizeInit, OAuthTokens, authorize_url, create_api_key, exchange_code,
    refresh,
};
pub use store::{CredentialsStore, ProviderCredentials};

#[derive(Debug)]
pub enum AuthError {
    MissingHomeDir,
    Io(String),
    Json(String),
    Http(String),
    InvalidResponse(String),
    Other(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::MissingHomeDir => write!(f, "Home directory not set"),
            AuthError::Io(msg) => write!(f, "Auth IO error: {msg}"),
            AuthError::Json(msg) => write!(f, "Auth JSON error: {msg}"),
            AuthError::Http(msg) => write!(f, "Auth HTTP error: {msg}"),
            AuthError::InvalidResponse(msg) => write!(f, "Auth response error: {msg}"),
            AuthError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<std::io::Error> for AuthError {
    fn from(error: std::io::Error) -> Self {
        AuthError::Io(error.to_string())
    }
}

impl From<serde_json::Error> for AuthError {
    fn from(error: serde_json::Error) -> Self {
        AuthError::Json(error.to_string())
    }
}

impl From<reqwest::Error> for AuthError {
    fn from(error: reqwest::Error) -> Self {
        AuthError::Http(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AuthError>;
