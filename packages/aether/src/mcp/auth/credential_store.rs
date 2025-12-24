use super::error::OAuthError;
use rmcp::transport::auth::{CredentialStore, StoredCredentials};
use std::path::PathBuf;
use tokio::fs;

/// File-based credential store that persists OAuth credentials to disk
pub struct FileCredentialStore {
    path: PathBuf,
}

impl FileCredentialStore {
    pub fn new(server_id: &str) -> Result<Self, OAuthError> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| {
                OAuthError::CredentialStore("Unable to determine home directory".to_string())
            })?;

        let credentials_dir = PathBuf::from(home)
            .join(".aether")
            .join("mcp-credentials");

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&credentials_dir)?;

        let path = credentials_dir.join(format!("{server_id}.json"));

        Ok(Self { path })
    }

    /// Create a credential store with a custom path (useful for testing)
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait::async_trait]
impl CredentialStore for FileCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, rmcp::transport::AuthError> {
        match fs::read_to_string(&self.path).await {
            Ok(content) => {
                let credentials = serde_json::from_str(&content).map_err(|e| {
                    rmcp::transport::AuthError::InternalError(format!(
                        "Failed to parse credentials: {e}"
                    ))
                })?;
                Ok(Some(credentials))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(rmcp::transport::AuthError::InternalError(format!(
                "Failed to read credentials: {e}"
            ))),
        }
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), rmcp::transport::AuthError> {
        let content = serde_json::to_string_pretty(&credentials).map_err(|e| {
            rmcp::transport::AuthError::InternalError(format!("Failed to serialize credentials: {e}"))
        })?;

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                rmcp::transport::AuthError::InternalError(format!(
                    "Failed to create credentials directory: {e}"
                ))
            })?;
        }

        fs::write(&self.path, content).await.map_err(|e| {
            rmcp::transport::AuthError::InternalError(format!("Failed to write credentials: {e}"))
        })?;

        Ok(())
    }

    async fn clear(&self) -> Result<(), rmcp::transport::AuthError> {
        match fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(rmcp::transport::AuthError::InternalError(format!(
                "Failed to clear credentials: {e}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oauth2::{
        AccessToken, RefreshToken, TokenResponse as _,
        basic::BasicTokenType,
        EmptyExtraTokenFields,
    };
    use std::time::Duration;

    fn create_test_token_response() -> oauth2::StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType> {
        let mut token_response = oauth2::StandardTokenResponse::new(
            AccessToken::new("test-token".to_string()),
            BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        token_response.set_expires_in(Some(&Duration::from_secs(3600)));
        token_response.set_refresh_token(Some(RefreshToken::new("refresh-token".to_string())));
        token_response
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test-server.json");
        let store = FileCredentialStore::with_path(path.clone());

        let credentials = StoredCredentials {
            client_id: "test-client".to_string(),
            token_response: Some(create_test_token_response()),
        };

        // Save credentials
        store.save(credentials.clone()).await.unwrap();

        // Load credentials
        let loaded = store.load().await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.client_id, "test-client");
        assert!(loaded.token_response.is_some());
        let token = loaded.token_response.unwrap();
        assert_eq!(token.access_token().secret(), "test-token");
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("nonexistent.json");
        let store = FileCredentialStore::with_path(path);

        let loaded = store.load().await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_clear() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test-server.json");
        let store = FileCredentialStore::with_path(path.clone());

        let credentials = StoredCredentials {
            client_id: "test-client".to_string(),
            token_response: Some(create_test_token_response()),
        };

        // Save and then clear
        store.save(credentials).await.unwrap();
        assert!(path.exists());

        store.clear().await.unwrap();
        assert!(!path.exists());

        // Clearing again should succeed
        store.clear().await.unwrap();
    }
}
