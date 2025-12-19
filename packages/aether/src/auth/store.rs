use crate::auth::{AuthError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CredentialsStore {
    pub version: u32,
    pub providers: HashMap<String, ProviderCredentials>,
}

impl CredentialsStore {
    pub fn empty() -> Self {
        Self {
            version: 1,
            providers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProviderCredentials {
    Api {
        key: String,
    },
    OAuth {
        access: String,
        refresh: String,
        expires: u64,
    },
}

impl ProviderCredentials {
    pub fn api_key(key: &str) -> Self {
        Self::Api {
            key: key.to_string(),
        }
    }
}

pub fn load() -> Result<CredentialsStore> {
    let path = path()?;
    if !path.exists() {
        return Ok(CredentialsStore::empty());
    }

    let bytes = fs::read(path)?;
    let store: CredentialsStore = serde_json::from_slice(&bytes)?;
    Ok(store)
}

pub fn save(store: &CredentialsStore) -> Result<()> {
    let path = path()?;
    let parent = path
        .parent()
        .ok_or_else(|| AuthError::Other("Invalid credentials path".to_string()))?;
    fs::create_dir_all(parent)?;

    let mut temp_file = tempfile::NamedTempFile::new_in(parent)?;
    let payload = serde_json::to_vec_pretty(store)?;
    temp_file.write_all(&payload)?;
    temp_file.write_all(b"\n")?;
    set_permissions(temp_file.path())?;
    temp_file.persist(&path).map_err(|error| error.error)?;
    set_permissions(&path)?;
    Ok(())
}

pub fn path() -> Result<PathBuf> {
    let base = match env::var("AETHER_HOME") {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => {
            let home = env::var("HOME")
                .or_else(|_| env::var("USERPROFILE"))
                .map_err(|_| AuthError::MissingHomeDir)?;
            PathBuf::from(home).join(".aether")
        }
    };

    Ok(base.join("credentials.json"))
}

fn set_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_temp_home<F>(test: F)
    where
        F: FnOnce(&Path),
    {
        let _guard = env_lock().lock().unwrap();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        unsafe {
            env::set_var("AETHER_HOME", temp_dir.path());
        }
        test(temp_dir.path());
        unsafe {
            env::remove_var("AETHER_HOME");
        }
    }

    #[test]
    fn load_missing_returns_empty() {
        with_temp_home(|_| {
            let store = load().expect("load");
            assert_eq!(store, CredentialsStore::empty());
        });
    }

    #[test]
    fn save_and_reload_roundtrip() {
        with_temp_home(|_| {
            let mut store = CredentialsStore::empty();
            store.providers.insert(
                "anthropic".to_string(),
                ProviderCredentials::api_key("sk-ant-test"),
            );
            save(&store).expect("save");

            let loaded = load().expect("load");
            assert_eq!(loaded, store);
        });
    }

    #[test]
    #[cfg(unix)]
    fn saves_with_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;
        with_temp_home(|_| {
            let store = CredentialsStore::empty();
            save(&store).expect("save");

            let path = path().expect("path");
            let metadata = fs::metadata(path).expect("metadata");
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        });
    }
}
