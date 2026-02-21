//! LSP Registry - manages LSP daemon clients with lazy connection
//!
//! The registry lazily connects to the LSP daemon on first access for each language.
//! LSP server configurations are managed by the daemon (`aether-lspd`).
//!
//! # Architecture
//!
//! Agents connect to a shared daemon (`aether-lspd`) that manages LSP servers.
//! This avoids spawning duplicate LSP servers when running multiple agents
//! concurrently.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aether_lspd::{LanguageId, LspClient, get_config_for_language};
use futures::future::join_all;

/// Registry that manages LSP daemon clients, connecting lazily on demand
pub struct LspRegistry {
    /// Active daemon clients keyed by language
    clients: tokio::sync::RwLock<HashMap<LanguageId, Arc<LspClient>>>,
    /// The project root directory
    root_path: PathBuf,
}

impl LspRegistry {
    /// Create a new registry for the given project root
    ///
    /// LSP server configurations are managed by the daemon.
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            clients: tokio::sync::RwLock::new(HashMap::new()),
            root_path,
        }
    }

    /// Get the project root path
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Get or connect to the LSP daemon client for a file path
    ///
    /// Returns None if no LSP is configured for this file type or if connection fails.
    pub async fn get_or_spawn(&self, file_path: &Path) -> Option<Arc<LspClient>> {
        let language_id = LanguageId::from_path(file_path);
        self.get_or_spawn_for_language(language_id).await
    }

    /// Get or connect to the LSP daemon client for a specific language
    ///
    /// Returns None if no LSP is configured for this language or if connection fails.
    pub async fn get_or_spawn_for_language(
        &self,
        language_id: LanguageId,
    ) -> Option<Arc<LspClient>> {
        // Check if daemon has config for this language
        get_config_for_language(language_id)?;

        // Check if already connected
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(&language_id) {
                return Some(Arc::clone(client));
            }
        }

        // Connect to daemon
        let mut clients = self.clients.write().await;

        // Double-check after acquiring write lock
        if let Some(client) = clients.get(&language_id) {
            return Some(Arc::clone(client));
        }

        match LspClient::connect_or_spawn(&self.root_path, language_id).await {
            Ok(client) => {
                let client = Arc::new(client);
                clients.insert(language_id, Arc::clone(&client));
                Some(client)
            }
            Err(e) => {
                tracing::error!(
                    "Failed to connect to LSP daemon for {:?}: {}",
                    language_id,
                    e
                );
                None
            }
        }
    }

    /// Get the LSP daemon client for a specific language, if already connected
    pub async fn get_client_for_language(&self, language_id: LanguageId) -> Option<Arc<LspClient>> {
        let clients = self.clients.read().await;
        clients.get(&language_id).cloned()
    }

    /// Get all active LSP daemon clients
    pub async fn active_clients(&self) -> Vec<Arc<LspClient>> {
        let clients = self.clients.read().await;
        clients.values().cloned().collect()
    }

    /// Check if an LSP is configured for a given file path
    ///
    /// This checks the daemon's configuration registry.
    pub fn has_config_for(&self, file_path: &Path) -> bool {
        let language_id = LanguageId::from_path(file_path);
        get_config_for_language(language_id).is_some()
    }

    /// Connect to LSP daemon for all detected project languages.
    ///
    /// This scans the project root for manifest files (Cargo.toml, package.json, etc.)
    /// and connects to the LSP daemon for each detected language. Designed to be called
    /// at boot time so LSPs can start indexing immediately.
    pub async fn spawn_project_lsps(&self) {
        let languages = self.detect_project_languages();
        let spawn_futures: Vec<_> = languages
            .iter()
            .map(|&lang| async move { (lang, self.get_or_spawn_for_language(lang).await) })
            .collect();

        for (lang, result) in join_all(spawn_futures).await {
            if result.is_some() {
                tracing::info!(
                    "Connected to LSP daemon for {:?} based on project detection",
                    lang
                );
            }
        }
    }

    /// Detect project languages by checking for manifest files in the root directory.
    ///
    /// This is a fast, synchronous check that looks for common project files:
    /// - Cargo.toml → Rust
    /// - package.json → TypeScript/JavaScript
    /// - pyproject.toml / setup.py / requirements.txt → Python
    /// - go.mod → Go
    /// - CMakeLists.txt → C/C++
    fn detect_project_languages(&self) -> Vec<LanguageId> {
        let mappings = [
            (LanguageId::Rust, HashSet::from(["Cargo.toml"])),
            (LanguageId::TypeScript, HashSet::from(["package.json"])),
            (LanguageId::Go, HashSet::from(["go.mod"])),
            (LanguageId::Cpp, HashSet::from(["CMakeLists.txt"])),
            (
                LanguageId::Python,
                HashSet::from(["pyproject.toml", "setup.py", "requirements.txt"]),
            ),
        ];

        mappings
            .iter()
            .filter(|(_, files)| files.iter().any(|f| self.root_path.join(f).exists()))
            .map(|(lang, _)| *lang)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_config_for() {
        let registry = LspRegistry::new(PathBuf::from("/tmp"));

        assert!(registry.has_config_for(Path::new("foo.rs")));
        assert!(registry.has_config_for(Path::new("bar.ts")));
        assert!(registry.has_config_for(Path::new("baz.py")));
        assert!(!registry.has_config_for(Path::new("unknown.xyz")));
    }

    #[tokio::test]
    async fn test_no_clients_initially() {
        let registry = LspRegistry::new(PathBuf::from("/tmp"));

        assert!(registry.active_clients().await.is_empty());
    }
}
