use crate::auth::{FileCredentialStore, RmcpCredentialStoreAdapter};
use crate::mcp::auth::{OAuthError, OAuthHandler};
use rmcp::transport::auth::{AuthorizationManager, OAuthState};

pub struct OAuthHelperResult {
    pub access_token: String,
    pub auth_header: String,
}

/// Create a credential store adapter for the given server.
fn create_credential_store(server_id: &str) -> Result<RmcpCredentialStoreAdapter, OAuthError> {
    let store = FileCredentialStore::new()
        .map_err(|e| OAuthError::CredentialStore(format!("Failed to create store: {e}")))?;
    Ok(RmcpCredentialStoreAdapter::new(store, server_id))
}

/// Initialize an AuthorizationManager from stored credentials.
///
/// Creates the credential store, sets it on the manager, and calls `initialize_from_store()`
/// which discovers OAuth metadata and configures the client properly.
///
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

pub async fn perform_oauth_flow<H: OAuthHandler>(
    server_id: &str,
    base_url: &str,
    handler: &H,
    redirect_uri: &str,
    scopes: &[&str],
) -> Result<OAuthHelperResult, OAuthError> {
    // Try to load existing credentials first
    if let Some(auth_manager) = create_auth_manager_from_store(server_id, base_url).await? {
        if let Ok(access_token) = auth_manager.get_access_token().await {
            return Ok(OAuthHelperResult {
                access_token: access_token.clone(),
                auth_header: format!("Bearer {access_token}"),
            });
        }
        // Token might be expired and refresh failed, continue to new auth flow
    }

    // No stored credentials or they're invalid, start new OAuth flow
    let credential_store = create_credential_store(server_id)?;
    let mut oauth_state = OAuthState::new(base_url, None)
        .await
        .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

    // Configure credential store
    match oauth_state {
        OAuthState::Unauthorized(ref mut manager) => {
            manager.set_credential_store(credential_store);
        }
        _ => {
            return Err(OAuthError::Rmcp("Expected Unauthorized state".to_string()));
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

    // Call the handler to get authorization code and state (CSRF token)
    let callback = handler.authorize(&auth_url).await?;

    // Exchange code for token with CSRF validation
    oauth_state
        .handle_callback(&callback.code, &callback.state)
        .await
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?;

    // Get access token from the authorized manager
    let manager = oauth_state
        .into_authorization_manager()
        .ok_or_else(|| OAuthError::Rmcp("Failed to get authorization manager".to_string()))?;

    let access_token = manager
        .get_access_token()
        .await
        .map_err(|e| OAuthError::Rmcp(e.to_string()))?;

    Ok(OAuthHelperResult {
        access_token: access_token.clone(),
        auth_header: format!("Bearer {access_token}"),
    })
}

pub async fn get_access_token_for_server(
    server_id: &str,
    base_url: &str,
) -> Result<Option<String>, OAuthError> {
    match create_auth_manager_from_store(server_id, base_url).await? {
        Some(auth_manager) => match auth_manager.get_access_token().await {
            Ok(token) => Ok(Some(token)),
            Err(_) => Ok(None),
        },
        None => Ok(None),
    }
}
