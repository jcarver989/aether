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
}
