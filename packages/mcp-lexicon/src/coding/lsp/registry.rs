//! LSP Registry - manages multiple LSP clients with lazy spawning
//!
//! The registry maps file extensions/languages to LSP configurations and
//! lazily spawns LSP servers on first access. This allows efficient resource
//! usage by only starting LSP servers for languages actually being used.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::client::{LspClient, NotificationReceiver, NotificationSender, ServerNotification};
use super::config::LspConfig;
use super::transport::LanguageId;
use futures::future::join_all;
use lsp_types::{Diagnostic, PublishDiagnosticsParams};
use tokio::spawn;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// Request to query the aggregated diagnostics from all LSPs
type DiagnosticsQuery = oneshot::Sender<HashMap<String, Vec<Diagnostic>>>;

/// Handle to an LSP client and its associated state
pub struct LspClientHandle {
    /// The LSP client for making requests
    pub client: Arc<LspClient>,
    /// Notification sender for the LSP client
    pub notification_tx: NotificationSender,
    /// Channel to query diagnostics from this client's cache
    diagnostics_query_tx: mpsc::Sender<DiagnosticsQuery>,
    /// Handle to the cache actor task (kept alive)
    _cache_task: JoinHandle<()>,
}

impl LspClientHandle {
    /// Create a new client handle, spawning the diagnostics cache actor
    fn new(
        client: LspClient,
        notification_tx: NotificationSender,
        notification_rx: NotificationReceiver,
    ) -> Self {
        let (query_tx, query_rx) = mpsc::channel(16);
        let cache_task = spawn(run_cache_actor(notification_rx, query_rx));

        Self {
            client: Arc::new(client),
            notification_tx,
            diagnostics_query_tx: query_tx,
            _cache_task: cache_task,
        }
    }

    /// Get all cached diagnostics from this LSP client
    pub async fn get_diagnostics(&self) -> Result<HashMap<String, Vec<Diagnostic>>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.diagnostics_query_tx
            .send(response_tx)
            .await
            .map_err(|_| "Failed to query diagnostics cache")?;
        response_rx
            .await
            .map_err(|_| "Diagnostics query channel closed".to_string())
    }
}

/// Actor task that owns the diagnostics cache for a single LSP
async fn run_cache_actor(
    mut notification_rx: NotificationReceiver,
    mut query_rx: mpsc::Receiver<DiagnosticsQuery>,
) {
    let mut cache: HashMap<String, Vec<Diagnostic>> = HashMap::new();

    loop {
        tokio::select! {
            Some(notification) = notification_rx.recv() => {
                if let ServerNotification::Diagnostics(PublishDiagnosticsParams { uri, diagnostics, .. }) = notification {
                    cache.insert(uri.to_string(), diagnostics);
                }
            }
            Some(response_tx) = query_rx.recv() => {
                let _ = response_tx.send(cache.clone());
            }
            else => break,
        }
    }
}

/// Registry that manages multiple LSP clients, spawning them lazily on demand
pub struct LspRegistry {
    /// Map from LanguageId to LSP configuration
    configs: HashMap<LanguageId, LspConfig>,
    /// Active LSP clients keyed by command name (since one LSP can serve multiple languages)
    clients: RwLock<HashMap<String, Arc<LspClientHandle>>>,
    /// The project root directory
    root_path: PathBuf,
}

impl LspRegistry {
    /// Create a new registry with the given configurations
    pub fn new(root_path: PathBuf, configs: Vec<LspConfig>) -> Self {
        let mut config_map = HashMap::new();
        for config in configs {
            for lang in &config.languages {
                config_map.insert(*lang, config.clone());
            }
        }

        Self {
            configs: config_map,
            clients: RwLock::new(HashMap::new()),
            root_path,
        }
    }

    /// Get or spawn the LSP client for a file path
    ///
    /// Returns None if no LSP is configured for this file type or if spawning fails.
    pub async fn get_or_spawn(&self, file_path: &Path) -> Option<Arc<LspClientHandle>> {
        let language_id = LanguageId::from_path(file_path);
        self.get_or_spawn_for_language(language_id).await
    }

    /// Get or spawn the LSP client for a specific language
    ///
    /// Returns None if no LSP is configured for this language or if spawning fails.
    pub async fn get_or_spawn_for_language(
        &self,
        language_id: LanguageId,
    ) -> Option<Arc<LspClientHandle>> {
        let config = self.configs.get(&language_id)?;
        {
            let clients = self.clients.read().await;
            if let Some(handle) = clients.get(&config.command) {
                return Some(Arc::clone(handle));
            }
        }

        let mut clients = self.clients.write().await;
        if let Some(handle) = clients.get(&config.command) {
            return Some(Arc::clone(handle));
        }

        match LspClient::spawn(&config.command, &config.args_as_refs(), &self.root_path).await {
            Ok((tx, rx, client)) => {
                let handle = Arc::new(LspClientHandle::new(client, tx, rx));
                clients.insert(config.command.clone(), Arc::clone(&handle));
                Some(handle)
            }
            Err(e) => {
                tracing::error!("Failed to spawn LSP '{}': {}", config.command, e);
                None
            }
        }
    }

    /// Get the LSP client handle for a specific language, if already spawned
    pub async fn get_client_for_language(&self, lang: LanguageId) -> Option<Arc<LspClientHandle>> {
        let config = self.configs.get(&lang)?;
        let clients = self.clients.read().await;
        clients.get(&config.command).cloned()
    }

    /// Get all active LSP client handles
    pub async fn active_clients(&self) -> Vec<Arc<LspClientHandle>> {
        let clients = self.clients.read().await;
        clients.values().cloned().collect()
    }

    /// Check if an LSP is configured for a given file path
    pub fn has_config_for(&self, file_path: &Path) -> bool {
        let language_id = LanguageId::from_path(file_path);
        self.configs.contains_key(&language_id)
    }

    /// Get the configuration for a file extension (for testing)
    #[cfg(test)]
    pub fn config_for_language(&self, lang: LanguageId) -> Option<&LspConfig> {
        self.configs.get(&lang)
    }

    /// Spawn LSP servers for all detected project languages.
    ///
    /// This scans the project root for manifest files (Cargo.toml, package.json, etc.)
    /// and spawns appropriate LSP servers. Designed to be called at boot time so
    /// LSPs can start indexing immediately.
    pub async fn spawn_project_lsps(&self) {
        let languages = self.detect_project_languages();
        let spawn_futures: Vec<_> = languages
            .iter()
            .map(|&lang| async move { (lang, self.get_or_spawn_for_language(lang).await) })
            .collect();

        for (lang, result) in join_all(spawn_futures).await {
            if result.is_some() {
                tracing::info!("Spawned LSP for {:?} based on project detection", lang);
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
    use crate::coding::lsp::config::default_lsp_configs;

    #[test]
    fn test_language_to_config_mapping() {
        let registry = LspRegistry::new(PathBuf::from("/tmp"), default_lsp_configs());

        // Rust should map to rust-analyzer
        let rust_config = registry.config_for_language(LanguageId::Rust);
        assert!(rust_config.is_some());
        assert_eq!(rust_config.unwrap().command, "rust-analyzer");

        // TypeScript should map to typescript-language-server
        let ts_config = registry.config_for_language(LanguageId::TypeScript);
        assert!(ts_config.is_some());
        assert_eq!(ts_config.unwrap().command, "typescript-language-server");

        // Python should map to pyright-langserver
        let py_config = registry.config_for_language(LanguageId::Python);
        assert!(py_config.is_some());
        assert_eq!(py_config.unwrap().command, "pyright-langserver");

        // PlainText should have no config
        let txt_config = registry.config_for_language(LanguageId::PlainText);
        assert!(txt_config.is_none());
    }

    #[test]
    fn test_has_config_for() {
        let registry = LspRegistry::new(PathBuf::from("/tmp"), default_lsp_configs());

        assert!(registry.has_config_for(Path::new("foo.rs")));
        assert!(registry.has_config_for(Path::new("bar.ts")));
        assert!(registry.has_config_for(Path::new("baz.py")));
        assert!(!registry.has_config_for(Path::new("unknown.xyz")));
    }

    #[tokio::test]
    async fn test_no_clients_initially() {
        let registry = LspRegistry::new(PathBuf::from("/tmp"), default_lsp_configs());

        // No clients should be spawned yet
        assert!(registry.active_clients().await.is_empty());
    }
}
