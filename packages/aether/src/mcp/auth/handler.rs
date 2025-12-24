use super::error::OAuthError;
use std::future::Future;

/// Trait that consuming applications implement to handle OAuth UI/UX
pub trait OAuthHandler: Send + Sync {
    /// Called when user needs to authorize. App should open browser to `auth_url`
    /// and return the authorization code from the callback.
    fn authorize(&self, auth_url: &str) -> impl Future<Output = Result<String, OAuthError>> + Send;
}

/// Channel-based implementation for async workflows
pub struct ChannelOAuthHandler {
    auth_url_tx: tokio::sync::mpsc::Sender<String>,
    code_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<String>>,
}

impl ChannelOAuthHandler {
    pub fn new() -> (Self, ChannelOAuthHandlerSender) {
        let (auth_url_tx, auth_url_rx) = tokio::sync::mpsc::channel(1);
        let (code_tx, code_rx) = tokio::sync::mpsc::channel(1);

        let handler = Self {
            auth_url_tx,
            code_rx: tokio::sync::Mutex::new(code_rx),
        };

        let sender = ChannelOAuthHandlerSender { auth_url_rx, code_tx };

        (handler, sender)
    }
}

impl OAuthHandler for ChannelOAuthHandler {
    async fn authorize(&self, auth_url: &str) -> Result<String, OAuthError> {
        // Send the auth URL to the application
        self.auth_url_tx
            .send(auth_url.to_string())
            .await
            .map_err(|_| OAuthError::UserCancelled)?;

        // Wait for the authorization code
        let mut rx = self.code_rx.lock().await;
        rx.recv()
            .await
            .ok_or(OAuthError::UserCancelled)
    }
}

/// The sender side for the channel-based handler
pub struct ChannelOAuthHandlerSender {
    auth_url_rx: tokio::sync::mpsc::Receiver<String>,
    code_tx: tokio::sync::mpsc::Sender<String>,
}

impl ChannelOAuthHandlerSender {
    /// Wait for an auth URL to be sent
    pub async fn recv_auth_url(&mut self) -> Option<String> {
        self.auth_url_rx.recv().await
    }

    /// Send the authorization code back
    pub async fn send_code(&self, code: String) -> Result<(), OAuthError> {
        self.code_tx
            .send(code)
            .await
            .map_err(|_| OAuthError::UserCancelled)
    }
}
