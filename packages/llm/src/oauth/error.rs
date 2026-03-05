use thiserror::Error;

#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("User cancelled authorization")]
    UserCancelled,

    #[error("Credential storage error: {0}")]
    CredentialStore(String),

    #[error("rmcp auth error: {0}")]
    Rmcp(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid OAuth callback: {0}")]
    InvalidCallback(String),

    #[error("Invalid JWT: {0}")]
    InvalidJwt(String),

    #[error("Token exchange failed: {0}")]
    TokenExchange(String),

    #[error("OAuth state mismatch — possible CSRF attack")]
    StateMismatch,

    #[error("No credentials found: {0}")]
    NoCredentials(String),
}
