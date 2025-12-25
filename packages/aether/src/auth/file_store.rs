use crate::auth::credentials::{CredentialsFile, McpCredential, ProviderCredential};
use crate::auth::{AuthError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// Async file-based credential store
///
/// Stores credentials in `~/.aether/credentials.json` with the following structure:
/// - `providers`: LLM provider credentials (API keys or OAuth tokens)
/// - `mcp_servers`: MCP server OAuth credentials
#[derive(Clone)]
pub struct FileCredentialStore {
    path: PathBuf,
    cache: Arc<RwLock<Option<CredentialsFile>>>,
}

impl FileCredentialStore {
    /// Create a new credential store at the default path (`~/.aether/credentials.json`)
    pub fn new() -> Result<Self> {
        let path = default_path()?;
        Ok(Self::with_path(path))
    }

    /// Create a credential store with a custom path (useful for testing)
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            path,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Get a provider credential by name
    pub async fn get_provider(&self, name: &str) -> Result<Option<ProviderCredential>> {
        let file = self.load_file().await?;
        Ok(file.providers.get(name).cloned())
    }

    /// Set a provider credential
    pub async fn set_provider(&self, name: &str, credential: ProviderCredential) -> Result<()> {
        let mut file = self.load_file().await?;
        file.providers.insert(name.to_string(), credential);
        self.save_file(&file).await
    }

    /// Remove a provider credential
    pub async fn remove_provider(&self, name: &str) -> Result<()> {
        let mut file = self.load_file().await?;
        file.providers.remove(name);
        self.save_file(&file).await
    }

    /// Get an MCP server credential by server ID
    pub async fn get_mcp_server(&self, server_id: &str) -> Result<Option<McpCredential>> {
        let file = self.load_file().await?;
        Ok(file.mcp_servers.get(server_id).cloned())
    }

    /// Set an MCP server credential
    pub async fn set_mcp_server(&self, server_id: &str, credential: McpCredential) -> Result<()> {
        let mut file = self.load_file().await?;
        file.mcp_servers.insert(server_id.to_string(), credential);
        self.save_file(&file).await
    }

    /// Remove an MCP server credential
    pub async fn remove_mcp_server(&self, server_id: &str) -> Result<()> {
        let mut file = self.load_file().await?;
        file.mcp_servers.remove(server_id);
        self.save_file(&file).await
    }

    /// Load the credentials file
    async fn load_file(&self) -> Result<CredentialsFile> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(ref file) = *cache {
                return Ok(file.clone());
            }
        }

        // Load from disk
        let file = match fs::read_to_string(&self.path).await {
            Ok(content) => serde_json::from_str(&content)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => CredentialsFile::default(),
            Err(e) => return Err(AuthError::Io(e.to_string())),
        };

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(file.clone());
        }

        Ok(file)
    }

    /// Save the credentials file to disk
    async fn save_file(&self, file: &CredentialsFile) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(file)?;

        // Write atomically using a temp file
        let temp_path = self.path.with_extension("json.tmp");
        fs::write(&temp_path, &content).await?;
        set_permissions(&temp_path).await?;
        fs::rename(&temp_path, &self.path).await?;
        set_permissions(&self.path).await?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(file.clone());
        }

        Ok(())
    }

    /// Invalidate the cache, forcing a reload from disk on next access
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }
}

/// Get the default credentials file path
fn default_path() -> Result<PathBuf> {
    let base = match std::env::var("AETHER_HOME") {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map_err(|_| AuthError::MissingHomeDir)?;
            PathBuf::from(home).join(".aether")
        }
    };

    Ok(base.join("credentials.json"))
}

/// Set restrictive permissions on the credentials file (Unix only)
async fn set_permissions(path: &Path) -> Result<()> {
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
    async fn test_provider_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path);

        store
            .set_provider("anthropic", ProviderCredential::api_key("sk-test"))
            .await
            .unwrap();

        let loaded = store.get_provider("anthropic").await.unwrap();
        assert!(loaded.is_some());
        match loaded.unwrap() {
            ProviderCredential::ApiKey { key } => assert_eq!(key, "sk-test"),
            _ => panic!("Expected ApiKey"),
        }
    }

    #[tokio::test]
    async fn test_mcp_server_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path);

        let credential = McpCredential {
            client_id: "client-123".to_string(),
            access_token: "token-abc".to_string(),
            refresh_token: Some("refresh-xyz".to_string()),
            expires_at: Some(1703001600),
        };

        store
            .set_mcp_server("github-server", credential)
            .await
            .unwrap();

        let loaded = store.get_mcp_server("github-server").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.client_id, "client-123");
        assert_eq!(loaded.access_token, "token-abc");
    }

    #[tokio::test]
    async fn test_nonexistent_returns_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path);

        let provider = store.get_provider("nonexistent").await.unwrap();
        assert!(provider.is_none());

        let mcp = store.get_mcp_server("nonexistent").await.unwrap();
        assert!(mcp.is_none());
    }

    #[tokio::test]
    async fn test_remove_provider() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path);

        store
            .set_provider("test", ProviderCredential::api_key("key"))
            .await
            .unwrap();
        assert!(store.get_provider("test").await.unwrap().is_some());

        store.remove_provider("test").await.unwrap();
        assert!(store.get_provider("test").await.unwrap().is_none());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path.clone());

        store
            .set_provider("test", ProviderCredential::api_key("key"))
            .await
            .unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
