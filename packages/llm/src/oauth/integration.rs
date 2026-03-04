use super::OAuthError;
use super::credential_store::OAuthCredentialStore;
use super::handler::OAuthHandler;
use rmcp::transport::auth::{AuthClient, AuthorizationManager, OAuthState};

fn create_credential_store(server_id: &str) -> Result<OAuthCredentialStore, OAuthError> {
    OAuthCredentialStore::new(server_id)
}

/// Returns `Ok(Some(manager))` if credentials were found and initialized successfully,
/// `Ok(None)` if no stored credentials exist, or `Err` on failure.
pub async fn create_auth_manager_from_store(
    server_id: &str,
    base_url: &str,
) -> Result<Option<AuthorizationManager>, OAuthError> {
    let credential_store = create_credential_store(server_id)?;

    let mut auth_manager = AuthorizationManager::new(base_url)
        .await
        .map_err(|e| OAuthError::Rmcp(e.to_string()))?;
    auth_manager.set_credential_store(credential_store);

    if auth_manager
        .initialize_from_store()
        .await
        .map_err(|e| OAuthError::Rmcp(e.to_string()))?
    {
        Ok(Some(auth_manager))
    } else {
        Ok(None)
    }
}

/// Run a full OAuth authorization flow against an MCP server.
///
/// Creates the `OAuthState`, starts authorization using the handler's redirect URI,
/// opens the browser (via the handler), handles the callback, and returns an `AuthClient`
/// ready for authenticated HTTP transport.
pub async fn perform_oauth_flow(
    server_id: &str,
    base_url: &str,
    handler: &dyn OAuthHandler,
) -> Result<AuthClient<reqwest::Client>, OAuthError> {
    let mut oauth_state = OAuthState::new(base_url, None)
        .await
        .map_err(|e| OAuthError::Rmcp(format!("OAuth init failed: {e}")))?;

    let credential_store = create_credential_store(server_id)?;
    if let OAuthState::Unauthorized(ref mut manager) = oauth_state {
        manager.set_credential_store(credential_store);
    }

    oauth_state
        .start_authorization(&[], handler.redirect_uri(), Some(server_id))
        .await
        .map_err(|e| OAuthError::Rmcp(format!("start_authorization failed: {e}")))?;

    let auth_url = oauth_state
        .get_authorization_url()
        .await
        .map_err(|e| OAuthError::Rmcp(format!("get_authorization_url failed: {e}")))?;

    let callback = handler.authorize(&auth_url).await?;

    oauth_state
        .handle_callback(&callback.code, &callback.state)
        .await
        .map_err(|e| OAuthError::Rmcp(format!("handle_callback failed: {e}")))?;

    let auth_manager = oauth_state.into_authorization_manager().ok_or_else(|| {
        OAuthError::Rmcp("OAuth flow did not produce an AuthorizationManager".into())
    })?;

    Ok(AuthClient::new(reqwest::Client::default(), auth_manager))
}
