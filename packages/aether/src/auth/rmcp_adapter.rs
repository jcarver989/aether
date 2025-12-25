use crate::auth::credentials::McpCredential;
use crate::auth::file_store::FileCredentialStore;
use async_trait::async_trait;
use oauth2::{AccessToken, RefreshToken, TokenResponse};
use rmcp::transport::auth::{AuthError, CredentialStore, StoredCredentials};
use std::time::Duration;

/// Adapter that wraps FileCredentialStore to implement rmcp's CredentialStore trait
///
/// This allows the unified credential store to be used with rmcp's OAuth infrastructure.
pub struct RmcpCredentialStoreAdapter {
    store: FileCredentialStore,
    server_id: String,
}

impl RmcpCredentialStoreAdapter {
    pub fn new(store: FileCredentialStore, server_id: &str) -> Self {
        Self {
            store,
            server_id: server_id.to_string(),
        }
    }
}

#[async_trait]
impl CredentialStore for RmcpCredentialStoreAdapter {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        let credential = self
            .store
            .get_mcp_server(&self.server_id)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;

        match credential {
            Some(cred) => {
                let token_response = build_token_response(&cred);
                Ok(Some(StoredCredentials {
                    client_id: cred.client_id,
                    token_response: Some(token_response),
                }))
            }
            None => Ok(None),
        }
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        let token = credentials
            .token_response
            .ok_or_else(|| AuthError::InternalError("No token response to save".to_string()))?;

        let expires_at = token.expires_in().map(|duration| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
                + duration.as_millis() as u64
        });

        let credential = McpCredential {
            client_id: credentials.client_id,
            access_token: token.access_token().secret().to_string(),
            refresh_token: token.refresh_token().map(|t| t.secret().to_string()),
            expires_at,
        };

        self.store
            .set_mcp_server(&self.server_id, credential)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))
    }

    async fn clear(&self) -> Result<(), AuthError> {
        self.store
            .remove_mcp_server(&self.server_id)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))
    }
}

/// Build an oauth2 token response from our stored credential
fn build_token_response(
    cred: &McpCredential,
) -> oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType> {
    use oauth2::{EmptyExtraTokenFields, StandardTokenResponse, basic::BasicTokenType};

    let mut response = StandardTokenResponse::new(
        AccessToken::new(cred.access_token.clone()),
        BasicTokenType::Bearer,
        EmptyExtraTokenFields {},
    );

    if let Some(ref refresh) = cred.refresh_token {
        response.set_refresh_token(Some(RefreshToken::new(refresh.clone())));
    }

    if let Some(expires_at_millis) = cred.expires_at {
        let now_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if expires_at_millis > now_millis {
            response.set_expires_in(Some(&Duration::from_millis(expires_at_millis - now_millis)));
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path);
        let adapter = RmcpCredentialStoreAdapter::new(store, "test-server");

        // Create a token response
        let mut token_response = oauth2::StandardTokenResponse::new(
            AccessToken::new("test-access-token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            oauth2::EmptyExtraTokenFields {},
        );
        token_response.set_refresh_token(Some(RefreshToken::new("test-refresh-token".to_string())));
        token_response.set_expires_in(Some(&Duration::from_secs(3600)));

        let credentials = StoredCredentials {
            client_id: "test-client-id".to_string(),
            token_response: Some(token_response),
        };

        // Save and load
        adapter.save(credentials).await.unwrap();
        let loaded = adapter.load().await.unwrap();

        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.client_id, "test-client-id");
        assert!(loaded.token_response.is_some());

        let token = loaded.token_response.unwrap();
        assert_eq!(token.access_token().secret(), "test-access-token");
        assert_eq!(
            token.refresh_token().map(|t| t.secret().as_str()),
            Some("test-refresh-token")
        );
    }

    #[tokio::test]
    async fn test_adapter_clear() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path);
        let adapter = RmcpCredentialStoreAdapter::new(store, "test-server");

        // Save a credential
        let token_response = oauth2::StandardTokenResponse::new(
            AccessToken::new("token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            oauth2::EmptyExtraTokenFields {},
        );
        let credentials = StoredCredentials {
            client_id: "client".to_string(),
            token_response: Some(token_response),
        };
        adapter.save(credentials).await.unwrap();

        // Verify it exists
        assert!(adapter.load().await.unwrap().is_some());

        // Clear and verify it's gone
        adapter.clear().await.unwrap();
        assert!(adapter.load().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_adapter_load_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path);
        let adapter = RmcpCredentialStoreAdapter::new(store, "nonexistent");

        let loaded = adapter.load().await.unwrap();
        assert!(loaded.is_none());
    }
}
