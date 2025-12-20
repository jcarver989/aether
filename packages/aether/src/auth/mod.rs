use thiserror::Error;

pub mod anthropic;
pub mod store;

pub use anthropic::{
    AnthropicAuthMode, AuthorizeInit, OAuthTokens, authorize_url, create_api_key, exchange_code,
    refresh,
};
pub use store::{CredentialsStore, ProviderCredentials};

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Home directory not set")]
    MissingHomeDir,
    #[error("Auth IO error: {0}")]
    Io(String),
    #[error("Auth JSON error: {0}")]
    Json(String),
    #[error("Auth HTTP error: {0}")]
    Http(String),
    #[error("Auth response error: {0}")]
    InvalidResponse(String),
    #[error("{0}")]
    Other(String),
}

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
