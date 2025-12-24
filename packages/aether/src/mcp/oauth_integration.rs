#[cfg(feature = "mcp-oauth")]
use crate::mcp::auth::{FileCredentialStore, OAuthError, OAuthHandler};
#[cfg(feature = "mcp-oauth")]
use rmcp::transport::auth::{AuthorizationManager, CredentialStore, OAuthState};
#[cfg(feature = "mcp-oauth")]
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

#[cfg(feature = "mcp-oauth")]
pub struct OAuthHelperResult {
    pub access_token: String,
    pub auth_header: String,
}

#[cfg(feature = "mcp-oauth")]
pub async fn perform_oauth_flow<H: OAuthHandler>(
    server_id: &str,
    base_url: &str,
    handler: &H,
    redirect_uri: &str,
    scopes: &[&str],
) -> Result<OAuthHelperResult, OAuthError> {
    // Try to load existing credentials first
    {
        let credential_store = FileCredentialStore::new(server_id)?;
        if let Some(stored_creds) = credential_store.load().await.map_err(|e| {
            OAuthError::CredentialStore(format!("Failed to load credentials: {e}"))
        })? {
            if stored_creds.token_response.is_some() {
                // We have stored credentials, try to use them
                let mut auth_manager = AuthorizationManager::new(base_url)
                    .await
                    .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

                auth_manager.set_credential_store(credential_store);
                auth_manager
                    .configure_client_id(&stored_creds.client_id)
                    .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

                // Try to get access token (will refresh if needed)
                if let Ok(access_token) = auth_manager.get_access_token().await {
                    return Ok(OAuthHelperResult {
                        access_token: access_token.clone(),
                        auth_header: format!("Bearer {access_token}"),
                    });
                }
                // If we get here, token might be expired and refresh failed, continue to new auth flow
            }
        }
    }

    // No stored credentials or they're invalid, start new OAuth flow
    let credential_store = FileCredentialStore::new(server_id)?;
    let mut oauth_state = OAuthState::new(base_url, None)
        .await
        .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

    // Configure credential store
    match oauth_state {
        OAuthState::Unauthorized(ref mut manager) => {
            manager.set_credential_store(credential_store);
        }
        _ => {
            return Err(OAuthError::Rmcp(
                "Expected Unauthorized state".to_string(),
            ));
        }
    }

    // Start authorization
    oauth_state
        .start_authorization(scopes, redirect_uri, Some(server_id))
        .await
        .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

    // Get authorization URL
    let auth_url = oauth_state
        .get_authorization_url()
        .await
        .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

    // Call the handler to get authorization code
    let code = handler.authorize(&auth_url).await?;

    // Exchange code for token
    // Note: We need to extract the CSRF token from the URL that the handler got
    // For now, we'll use a simplified flow that doesn't validate CSRF
    // TODO: Improve this to properly handle CSRF tokens
    oauth_state
        .handle_callback(&code, "")
        .await
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?;

    // Get access token
    let access_token = oauth_state
        .get_access_token()
        .await
        .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

    Ok(OAuthHelperResult {
        access_token: access_token.clone(),
        auth_header: format!("Bearer {access_token}"),
    })
}

#[cfg(feature = "mcp-oauth")]
pub async fn get_access_token_for_server(server_id: &str, base_url: &str) -> Result<Option<String>, OAuthError> {
    // Create credential store
    let credential_store = FileCredentialStore::new(server_id)?;

    // Try to load existing credentials
    if let Some(stored_creds) = credential_store.load().await.map_err(|e| {
        OAuthError::CredentialStore(format!("Failed to load credentials: {e}"))
    })? {
        if stored_creds.token_response.is_some() {
            let mut auth_manager = AuthorizationManager::new(base_url)
                .await
                .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

            auth_manager.set_credential_store(credential_store);
            auth_manager
                .configure_client_id(&stored_creds.client_id)
                .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

            // Try to get access token (will refresh if needed)
            match auth_manager.get_access_token().await {
                Ok(access_token) => return Ok(Some(access_token)),
                Err(_) => {
                    // Token might be expired and refresh failed
                    return Ok(None);
                }
            }
        }
    }

    Ok(None)
}

#[cfg(feature = "mcp-oauth")]
pub fn update_config_with_auth(
    mut config: StreamableHttpClientTransportConfig,
    auth_header: String,
) -> StreamableHttpClientTransportConfig {
    config.auth_header = Some(auth_header);
    config
}
