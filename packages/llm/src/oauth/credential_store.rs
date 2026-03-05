use async_trait::async_trait;
use oauth2::{AccessToken, RefreshToken, TokenResponse};
use rmcp::transport::auth::{AuthError, CredentialStore, StoredCredentials};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::fs;

use super::OAuthError;

/// Credential for an OAuth provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredential {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix timestamp in milliseconds when the token expires
    pub expires_at: Option<u64>,
}

/// OAuth credential store that persists credentials to disk
/// and directly implements rmcp's `CredentialStore` trait.
///
/// Stores credentials in `~/.aether/mcp_credentials.json` keyed by server/provider ID.
#[derive(Clone)]
pub struct OAuthCredentialStore {
    server_id: String,
    path: PathBuf,
    cache: Arc<RwLock<Option<HashMap<String, OAuthCredential>>>>,
}

impl OAuthCredentialStore {
    /// Create a new store for the given server at the default path
    pub fn new(server_id: &str) -> Result<Self, OAuthError> {
        let path = default_path()?;
        Ok(Self {
            server_id: server_id.to_string(),
            path,
            cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Create a store with a custom path (useful for testing)
    pub fn with_path(server_id: &str, path: PathBuf) -> Self {
        Self {
            server_id: server_id.to_string(),
            path,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    async fn load_file(&self) -> Result<HashMap<String, OAuthCredential>, OAuthError> {
        if let Some(data) = self.cache.read().unwrap().clone() {
            return Ok(data);
        }

        let data = match fs::read_to_string(&self.path).await {
            Ok(content) => serde_json::from_str(&content)
                .map_err(|e| OAuthError::CredentialStore(e.to_string()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(e.into()),
        };

        *self.cache.write().unwrap() = Some(data.clone());
        Ok(data)
    }

    /// Load the raw `OAuthCredential` for this store's server ID.
    pub async fn load_credential(&self) -> Result<Option<OAuthCredential>, OAuthError> {
        let data = self.load_file().await?;
        Ok(data.get(&self.server_id).cloned())
    }

    /// Save a raw `OAuthCredential` directly, keyed by this store's server ID.
    ///
    /// This bypasses the rmcp `CredentialStore` trait and lets callers like
    /// the Codex OAuth flow save credentials without constructing a full
    /// `StoredCredentials` / `StandardTokenResponse`.
    pub async fn save_credential(&self, credential: OAuthCredential) -> Result<(), OAuthError> {
        let mut data = self.load_file().await?;
        data.insert(self.server_id.clone(), credential);
        self.save_file(&data).await
    }

    /// Check synchronously whether credentials exist for a given server ID.
    ///
    /// Reads the credential file from disk each time so that newly-saved
    /// credentials (e.g. from `aether auth codex`) are visible immediately.
    pub fn has_credentials_sync(server_id: &str) -> bool {
        load_file_sync()
            .map(|data| data.contains_key(server_id))
            .unwrap_or(false)
    }

    /// Return the set of server IDs that have stored credentials.
    ///
    /// Reads the credential file once, suitable for batch checks.
    pub fn credential_ids_sync() -> HashSet<String> {
        load_file_sync()
            .map(|data| data.into_keys().collect())
            .unwrap_or_default()
    }

    async fn save_file(&self, data: &HashMap<String, OAuthCredential>) -> Result<(), OAuthError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(data)
            .map_err(|e| OAuthError::CredentialStore(e.to_string()))?;

        // Write atomically using a temp file
        let temp_path = self.path.with_extension("json.tmp");
        fs::write(&temp_path, &content).await?;
        set_permissions(&temp_path).await?;
        fs::rename(&temp_path, &self.path).await?;
        set_permissions(&self.path).await?;

        *self.cache.write().unwrap() = Some(data.clone());
        Ok(())
    }
}

#[async_trait]
impl CredentialStore for OAuthCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        let data = self
            .load_file()
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;

        match data.get(&self.server_id) {
            Some(cred) => {
                let token_response = build_token_response(cred);
                Ok(Some(StoredCredentials {
                    client_id: cred.client_id.clone(),
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
            let now_ms = u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
            )
            .unwrap_or(u64::MAX);
            let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
            now_ms.saturating_add(duration_ms)
        });

        let credential = OAuthCredential {
            client_id: credentials.client_id,
            access_token: token.access_token().secret().clone(),
            refresh_token: token.refresh_token().map(|t| t.secret().clone()),
            expires_at,
        };

        let mut data = self
            .load_file()
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        data.insert(self.server_id.clone(), credential);
        self.save_file(&data)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))
    }

    async fn clear(&self) -> Result<(), AuthError> {
        let mut data = self
            .load_file()
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        data.remove(&self.server_id);
        self.save_file(&data)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))
    }
}

/// Build an oauth2 token response from our stored credential
fn build_token_response(
    cred: &OAuthCredential,
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
        let now_millis = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        )
        .unwrap_or(u64::MAX);

        if expires_at_millis > now_millis {
            response.set_expires_in(Some(&Duration::from_millis(expires_at_millis - now_millis)));
        }
    }

    response
}

/// Resolve the Aether home directory.
fn aether_home() -> Option<PathBuf> {
    match std::env::var("AETHER_HOME") {
        Ok(value) if !value.trim().is_empty() => Some(PathBuf::from(value)),
        _ => {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .ok()?;
            Some(PathBuf::from(home).join(".aether"))
        }
    }
}

/// Synchronously load and parse the credentials file.
fn load_file_sync() -> Result<HashMap<String, OAuthCredential>, OAuthError> {
    let path = default_path()?;
    let content = std::fs::read_to_string(&path)?;
    serde_json::from_str(&content).map_err(|e| OAuthError::CredentialStore(e.to_string()))
}

/// Get the default credentials file path
fn default_path() -> Result<PathBuf, OAuthError> {
    let base = aether_home()
        .ok_or_else(|| OAuthError::CredentialStore("Home directory not set".into()))?;
    Ok(base.join("mcp_credentials.json"))
}

/// Set restrictive permissions on the credentials file (Unix only)
async fn set_permissions(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, perms).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("mcp_credentials.json");
        let store = OAuthCredentialStore::with_path("test-server", path);

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
        store.save(credentials).await.unwrap();
        let loaded = store.load().await.unwrap();

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
    async fn test_clear() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("mcp_credentials.json");
        let store = OAuthCredentialStore::with_path("test-server", path);

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
        store.save(credentials).await.unwrap();

        // Verify it exists
        assert!(store.load().await.unwrap().is_some());

        // Clear and verify it's gone
        store.clear().await.unwrap();
        assert!(store.load().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("mcp_credentials.json");
        let store = OAuthCredentialStore::with_path("nonexistent", path);

        let loaded = store.load().await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_save_credential_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("mcp_credentials.json");
        let store = OAuthCredentialStore::with_path("codex", path);

        let credential = OAuthCredential {
            client_id: "my-client".to_string(),
            access_token: "my-access-token".to_string(),
            refresh_token: Some("my-refresh-token".to_string()),
            expires_at: Some(9999999999999),
        };

        store.save_credential(credential).await.unwrap();

        let loaded = store.load().await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.client_id, "my-client");
        let token = loaded.token_response.unwrap();
        assert_eq!(token.access_token().secret(), "my-access-token");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("mcp_credentials.json");
        let store = OAuthCredentialStore::with_path("test-server", path.clone());

        let token_response = oauth2::StandardTokenResponse::new(
            AccessToken::new("token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            oauth2::EmptyExtraTokenFields {},
        );
        let credentials = StoredCredentials {
            client_id: "client".to_string(),
            token_response: Some(token_response),
        };
        store.save(credentials).await.unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
