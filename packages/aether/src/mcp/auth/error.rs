use thiserror::Error;

#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("OAuth authorization required for {server_id}")]
    AuthorizationRequired { server_id: String },

    #[error("User cancelled authorization")]
    UserCancelled,

    #[error("Token exchange failed: {0}")]
    TokenExchange(String),

    #[error("Credential storage error: {0}")]
    CredentialStore(String),

    #[error("rmcp auth error: {0}")]
    Rmcp(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
}
