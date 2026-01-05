use aether::auth::{AuthError, OAuthCallback, open_browser, wait_for_callback};
use aether::mcp::auth::{OAuthError, OAuthHandler};
use aether::mcp::oauth_integration;
use std::time::Duration;

/// Default port for OAuth callback server.
pub const OAUTH_CALLBACK_PORT: u16 = 18234;

/// Desktop OAuth handler that opens browser and listens for callback on a local TCP port.
pub struct DesktopOAuthHandler {
    callback_port: u16,
}

impl DesktopOAuthHandler {
    pub fn new(callback_port: u16) -> Self {
        Self { callback_port }
    }

    /// Perform a complete OAuth flow.
    ///
    /// This method:
    /// 1. Checks for existing cached credentials
    /// 2. If needed, opens browser for authorization
    /// 3. Waits for the OAuth callback
    /// 4. Exchanges the code for tokens
    /// 5. Persists credentials to ~/.aether/credentials.json
    ///
    /// Returns the access token on success.
    pub async fn handle_oauth(
        &self,
        server_id: &str,
        base_url: &str,
        scopes: &[&str],
    ) -> Result<String, OAuthError> {
        let redirect_uri = format!("http://127.0.0.1:{}/callback", self.callback_port);
        let result =
            oauth_integration::perform_oauth_flow(server_id, base_url, self, &redirect_uri, scopes)
                .await?;
        Ok(result.access_token)
    }
}

impl OAuthHandler for DesktopOAuthHandler {
    async fn authorize(&self, auth_url: &str) -> Result<OAuthCallback, OAuthError> {
        open_browser(auth_url).map_err(auth_error_to_oauth)?;

        let callback = tokio::time::timeout(
            Duration::from_secs(300),
            wait_for_callback(self.callback_port),
        )
        .await
        .map_err(|_| OAuthError::Rmcp("OAuth callback timeout".into()))?
        .map_err(auth_error_to_oauth)?;

        Ok(callback)
    }
}

/// Convert AuthError to OAuthError
fn auth_error_to_oauth(e: AuthError) -> OAuthError {
    match e {
        AuthError::Io(msg) => OAuthError::Io(std::io::Error::other(msg)),
        _ => OAuthError::Rmcp(e.to_string()),
    }
}
