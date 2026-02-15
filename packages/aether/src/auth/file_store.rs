use crate::auth::credentials::ProviderCredential;
use crate::auth::{AuthError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::fs;

/// Internal file format for provider credentials.
///
/// Uses the `providers` key for backward compatibility with the old
/// unified credentials file that also stored MCP server credentials.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CredentialsFile {
    #[serde(default)]
    providers: HashMap<String, ProviderCredential>,
}

/// Async file-based credential store for LLM provider API keys.
///
/// Stores credentials in `~/.aether/credentials.json`.
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

    /// Load the credentials file
    async fn load_file(&self) -> Result<CredentialsFile> {
        if let Some(file) = self.cache.read().unwrap_or_else(|e| e.into_inner()).clone() {
            return Ok(file);
        }

        let file = match fs::read_to_string(&self.path).await {
            Ok(content) => serde_json::from_str(&content)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => CredentialsFile::default(),
            Err(e) => return Err(AuthError::Io(e.to_string())),
        };

        *self.cache.write().unwrap_or_else(|e| e.into_inner()) = Some(file.clone());

        Ok(file)
    }

    /// Save the credentials file to disk
    async fn save_file(&self, file: &CredentialsFile) -> Result<()> {
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

        *self.cache.write().unwrap_or_else(|e| e.into_inner()) = Some(file.clone());

        Ok(())
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
        }
    }

    #[tokio::test]
    async fn test_nonexistent_returns_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path);

        let provider = store.get_provider("nonexistent").await.unwrap();
        assert!(provider.is_none());
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
    async fn test_poisoned_lock_recovery() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("credentials.json");
        let store = FileCredentialStore::with_path(path);

        // Store a credential first
        store
            .set_provider("anthropic", ProviderCredential::api_key("sk-test"))
            .await
            .unwrap();

        // Poison the lock by panicking while holding a write guard
        let cache = store.cache.clone();
        let _ = std::thread::spawn(move || {
            let _guard = cache.write().unwrap();
            panic!("intentional panic to poison the lock");
        })
        .join();

        // The lock is now poisoned — operations should still work
        let loaded = store.get_provider("anthropic").await.unwrap();
        assert!(loaded.is_some());
        match loaded.unwrap() {
            ProviderCredential::ApiKey { key } => assert_eq!(key, "sk-test"),
        }
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
