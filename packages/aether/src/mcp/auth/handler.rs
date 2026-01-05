use super::error::OAuthError;
use crate::auth::OAuthCallback;
use std::future::Future;
use tokio::sync::oneshot;

/// Trait that consuming applications implement to handle OAuth UI/UX
pub trait OAuthHandler: Send + Sync {
    /// Called when user needs to authorize. App should open browser to `auth_url`
    /// and return the authorization code and state (CSRF token) from the callback.
    fn authorize(
        &self,
        auth_url: &str,
    ) -> impl Future<Output = Result<OAuthCallback, OAuthError>> + Send;
}

/// Channel-based implementation for async workflows
pub struct ChannelOAuthHandler {
    auth_url_tx: tokio::sync::mpsc::Sender<AuthRequest>,
}

struct AuthRequest {
    auth_url: String,
    callback_tx: oneshot::Sender<OAuthCallback>,
}

impl ChannelOAuthHandler {
    pub fn new() -> (Self, ChannelOAuthHandlerSender) {
        let (auth_url_tx, auth_url_rx) = tokio::sync::mpsc::channel(1);

        let handler = Self { auth_url_tx };
        let sender = ChannelOAuthHandlerSender { auth_url_rx };

        (handler, sender)
    }
}

impl OAuthHandler for ChannelOAuthHandler {
    async fn authorize(&self, auth_url: &str) -> Result<OAuthCallback, OAuthError> {
        let (callback_tx, callback_rx) = oneshot::channel();

        // Send the auth request to the application
        self.auth_url_tx
            .send(AuthRequest {
                auth_url: auth_url.to_string(),
                callback_tx,
            })
            .await
            .map_err(|_| OAuthError::UserCancelled)?;

        // Wait for the authorization callback
        callback_rx.await.map_err(|_| OAuthError::UserCancelled)
    }
}

/// The sender side for the channel-based handler
pub struct ChannelOAuthHandlerSender {
    auth_url_rx: tokio::sync::mpsc::Receiver<AuthRequest>,
}

impl ChannelOAuthHandlerSender {
    /// Wait for an auth request and return the URL and a sender for the callback
    pub async fn recv_auth_request(&mut self) -> Option<(String, oneshot::Sender<OAuthCallback>)> {
        self.auth_url_rx
            .recv()
            .await
            .map(|req| (req.auth_url, req.callback_tx))
    }
}
