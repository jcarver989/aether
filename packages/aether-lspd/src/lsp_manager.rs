use crate::error::{DaemonError, DaemonResult};
use crate::file_watcher::{FileWatcherBatch, FileWatcherHandle};
use crate::language_id::LanguageId;
use crate::protocol::{LspErrorResponse, LspNotification};
use crate::uri::{path_to_uri, uri_to_path};
use lsp_types::notification::{
    DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Initialized, Notification, PublishDiagnostics,
};
use lsp_types::request::{
    Initialize, RegisterCapability, Request, UnregisterCapability, WorkDoneProgressCreate,
};
use lsp_types::{
    CallHierarchyClientCapabilities, ClientCapabilities, DidChangeWatchedFilesClientCapabilities,
    DidChangeWatchedFilesParams, DocumentSymbolClientCapabilities,
    DynamicRegistrationClientCapabilities, GeneralClientCapabilities, GotoCapability,
    HoverClientCapabilities, InitializeParams, MarkupKind, PublishDiagnosticsClientCapabilities,
    PublishDiagnosticsParams, RegistrationParams, TextDocumentClientCapabilities, Uri,
    WorkspaceClientCapabilities,
};
use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::fs::read_to_string;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Notify, RwLock, mpsc, oneshot};
use tokio::task::JoinHandle;

/// After the diagnostics version first advances, wait this long with no new
/// `publishDiagnostics` before considering the result settled. Rust-analyzer
/// often sends a clearing (empty) publish immediately, followed by the real
/// diagnostics a few hundred milliseconds later. Use a comfortably larger
/// quiet period than the observed 100–500 ms gap so all-files diagnostics
/// queries don't race and return stale empty results.
const DIAGNOSTICS_SETTLE_DURATION: Duration = Duration::from_millis(600);

/// Key for identifying an LSP instance
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct LspKey {
    pub workspace_root: PathBuf,
    pub language: LanguageId,
}

/// Manager for LSP server instances
#[derive(Clone)]
pub struct LspManager {
    lsps: Arc<RwLock<HashMap<LspKey, Arc<LspHandle>>>>,
}

/// Handle to an active LSP server
pub struct LspHandle {
    /// Request sender (method, params, response channel)
    request_tx: mpsc::Sender<LspRequestEnvelope>,
    /// Notification sender (fire-and-forget)
    notification_tx: mpsc::Sender<LspNotification>,
    /// All known URIs with optional cached diagnostics.
    /// Key present with `None` = discovered, no diagnostics yet.
    /// Key present with `Some(params)` = has diagnostics.
    uri_state: Arc<RwLock<HashMap<Uri, Option<PublishDiagnosticsParams>>>>,
    /// Documents that have been opened with didOpen, with version/content tracking
    open_documents: Arc<RwLock<HashMap<Uri, OpenDocumentState>>>,
    /// Notified whenever new `PublishDiagnostics` arrive in the cache
    diagnostics_notify: Arc<Notify>,
    /// Monotonically increasing counter, bumped on every publishDiagnostics
    diagnostics_version: Arc<AtomicU64>,
    /// Background task handle
    _task: JoinHandle<()>,
}

/// Envelope for an LSP request with response channel
struct LspRequestEnvelope {
    method: String,
    params: Value,
    response_tx: oneshot::Sender<Result<Value, LspErrorResponse>>,
}

#[derive(Clone, Copy, Debug)]
struct OpenDocumentState {
    content_hash: u64,
}

enum SyncAction {
    Open,
    Change,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EnsureDocumentOpenOutcome {
    Synced(u64),
    Unchanged,
    Failed,
}

impl LspManager {
    /// Create a new LSP manager
    pub fn new() -> Self {
        Self {
            lsps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or spawn an LSP for the given workspace and language
    pub async fn get_or_spawn(
        &self,
        key: LspKey,
        command: &str,
        args: &[String],
    ) -> DaemonResult<Arc<LspHandle>> {
        if let Some(handle) = self.lsps.read().await.get(&key) {
            return Ok(Arc::clone(handle));
        }

        let mut lsps = self.lsps.write().await;
        if let Some(handle) = lsps.get(&key) {
            return Ok(Arc::clone(handle));
        }

        let handle = Arc::new(spawn_lsp(&key.workspace_root, command, args)?);
        lsps.insert(key, handle.clone());
        Ok(handle)
    }

    /// Shutdown all LSP instances
    pub async fn shutdown(&self) {
        self.lsps.write().await.clear();
    }
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LspHandle {
    /// Send a raw LSP request and wait for the JSON response.
    ///
    /// The daemon is a pure `(method, params) -> Value` passthrough.
    pub async fn request_raw(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, LspErrorResponse> {
        let (response_tx, response_rx) = oneshot::channel();
        let envelope = LspRequestEnvelope {
            method: method.to_string(),
            params,
            response_tx,
        };

        self.request_tx
            .send(envelope)
            .await
            .map_err(|_| LspErrorResponse {
                code: -1,
                message: "LSP handler closed".into(),
            })?;

        response_rx.await.map_err(|_| LspErrorResponse {
            code: -1,
            message: "Response channel closed".into(),
        })?
    }

    /// Get cached diagnostics
    ///
    /// If `uri` is Some, returns diagnostics for that file only.
    /// If `uri` is None, returns all cached diagnostics.
    pub async fn get_diagnostics(&self, uri: Option<&Uri>) -> Vec<PublishDiagnosticsParams> {
        let cache = self.uri_state.read().await;
        if let Some(u) = uri {
            let found: Vec<_> = cache
                .get(u)
                .and_then(|opt| opt.as_ref())
                .cloned()
                .into_iter()
                .collect();
            tracing::debug!(
                uri = %u.as_str(),
                cached_files = cache.len(),
                found = !found.is_empty(),
                diagnostics = found.first().map_or(0, |p| p.diagnostics.len()),
                "get_diagnostics: single-file lookup"
            );
            found
        } else {
            let all: Vec<_> = cache
                .values()
                .filter_map(|opt| opt.as_ref())
                .cloned()
                .collect();
            let total: usize = all.iter().map(|p| p.diagnostics.len()).sum();
            tracing::debug!(
                cached_files = cache.len(),
                total_diagnostics = total,
                "get_diagnostics: all-files lookup"
            );
            all
        }
    }

    /// Return all URIs the daemon knows about (from publishDiagnostics
    /// and didChangeWatchedFiles events).
    pub async fn cached_uris(&self) -> Vec<Uri> {
        self.uri_state.read().await.keys().cloned().collect()
    }

    /// Remove unreadable/stale URIs from both known URI tracking and diagnostics cache.
    pub(crate) async fn forget_known_uris(&self, uris: &[Uri]) {
        if uris.is_empty() {
            return;
        }

        let mut uri_state = self.uri_state.write().await;
        let mut removed = 0usize;
        for uri in uris {
            if uri_state.remove(uri).is_some() {
                removed += 1;
            }
        }
        drop(uri_state);

        let mut open_documents = self.open_documents.write().await;
        for uri in uris {
            open_documents.remove(uri);
        }

        tracing::debug!(
            requested = uris.len(),
            removed,
            "forget_known_uris: pruned stale/unreadable URIs"
        );
    }

    /// Send a notification to the LSP server (fire-and-forget)
    pub async fn send_notification(&self, notification: LspNotification) {
        tracing::debug!(method = %notification.method, "Queueing notification to LSP");
        if let Err(e) = self.notification_tx.send(notification).await {
            tracing::warn!(%e, "Failed to send notification — channel closed");
        }
    }

    /// Ensure a document is open before sending an LSP request that targets it.
    ///
    /// If the document hasn't been opened yet, reads it from disk and sends
    /// a `didOpen` notification. This allows agents to skip explicit didOpen
    /// management — the daemon handles it transparently.
    ///
    /// Returns `Some(version_before)` if the document was synced (opened or changed),
    /// so the caller can wait for fresh diagnostics. Returns `None` if no sync was needed
    /// or if syncing failed.
    pub async fn ensure_document_open(&self, uri: &Uri) -> Option<u64> {
        match self.ensure_document_open_with_outcome(uri).await {
            EnsureDocumentOpenOutcome::Synced(version_before) => Some(version_before),
            EnsureDocumentOpenOutcome::Unchanged | EnsureDocumentOpenOutcome::Failed => None,
        }
    }

    /// Ensure a document is open and return a detailed outcome.
    pub(crate) async fn ensure_document_open_with_outcome(
        &self,
        uri: &Uri,
    ) -> EnsureDocumentOpenOutcome {
        let file_path = uri_to_path(uri);
        tracing::debug!(uri = %uri.as_str(), %file_path, "ensure_document_open: reading file");

        let content = match read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(uri = %uri.as_str(), %file_path, %e, "Could not read file for auto-open");
                return EnsureDocumentOpenOutcome::Failed;
            }
        };

        let content_hash = hash_content(&content);
        let sync_action = {
            let mut open_docs = self.open_documents.write().await;
            match open_docs.get_mut(uri) {
                Some(state) if state.content_hash == content_hash => {
                    tracing::debug!(
                        uri = %uri.as_str(),
                        "ensure_document_open: content unchanged (hash match), no sync needed"
                    );
                    None
                }
                Some(state) => {
                    tracing::debug!(
                        uri = %uri.as_str(),
                        old_hash = state.content_hash,
                        new_hash = content_hash,
                        "ensure_document_open: content changed, will re-sync"
                    );
                    state.content_hash = content_hash;
                    Some(SyncAction::Change)
                }
                None => {
                    tracing::debug!(
                        uri = %uri.as_str(),
                        "ensure_document_open: new document, will open"
                    );
                    open_docs.insert(uri.clone(), OpenDocumentState { content_hash });
                    Some(SyncAction::Open)
                }
            }
        };

        let version_before = self.diagnostics_version.load(Ordering::Relaxed);

        match sync_action {
            Some(SyncAction::Open) => {
                tracing::debug!(
                    uri = %uri.as_str(),
                    version_before,
                    "ensure_document_open: sending didOpen + didSave"
                );
                self.send_open_and_save(uri, &file_path, content).await;
                EnsureDocumentOpenOutcome::Synced(version_before)
            }
            Some(SyncAction::Change) => {
                tracing::debug!(
                    uri = %uri.as_str(),
                    version_before,
                    "ensure_document_open: sending didClose + didOpen + didSave (content changed)"
                );

                let close_params = lsp_types::DidCloseTextDocumentParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                };
                let close_notification = LspNotification {
                    method: DidCloseTextDocument::METHOD.to_string(),
                    params: serde_json::to_value(&close_params).unwrap(),
                };
                self.send_notification(close_notification).await;

                self.send_open_and_save(uri, &file_path, content).await;
                EnsureDocumentOpenOutcome::Synced(version_before)
            }
            None => EnsureDocumentOpenOutcome::Unchanged,
        }
    }

    /// Close a document that was opened by `ensure_document_open`, releasing it
    /// back to file-watcher control.
    ///
    /// In the LSP protocol, once a file is `didOpen`'d the server ignores
    /// `didChangeWatchedFiles` for it — the client "owns" the document. Closing
    /// it lets the file watcher resume delivering external edits.
    pub async fn close_document(&self, uri: &Uri) {
        if self.open_documents.write().await.remove(uri).is_none() {
            tracing::debug!(uri = %uri.as_str(), "close_document: not in open_documents, skipping");
            return; // wasn't open
        }

        let close_params = lsp_types::DidCloseTextDocumentParams {
            text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        };
        let notification = LspNotification {
            method: DidCloseTextDocument::METHOD.to_string(),
            params: serde_json::to_value(&close_params).unwrap(),
        };
        self.send_notification(notification).await;
        tracing::debug!(uri = %uri.as_str(), "close_document: sent didClose, file watcher resumes");
    }

    /// Send `didOpen` + `didSave` notifications for a document.
    async fn send_open_and_save(&self, uri: &Uri, file_path: &str, content: String) {
        let language_id = LanguageId::from_path(Path::new(file_path));
        let open_params = lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: uri.clone(),
                language_id: language_id.as_str().to_string(),
                version: 1,
                text: content,
            },
        };
        self.send_notification(LspNotification {
            method: DidOpenTextDocument::METHOD.to_string(),
            params: serde_json::to_value(&open_params).unwrap(),
        })
        .await;

        let save_params = lsp_types::DidSaveTextDocumentParams {
            text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            text: None,
        };
        self.send_notification(LspNotification {
            method: DidSaveTextDocument::METHOD.to_string(),
            params: serde_json::to_value(&save_params).unwrap(),
        })
        .await;
    }

    /// Wait until the diagnostics version advances past `version_before` **and**
    /// the LSP has stopped publishing new diagnostics for [`DIAGNOSTICS_SETTLE_DURATION`].
    ///
    /// **Phase 1** — wait for the version to advance (existing behaviour).
    /// **Phase 2** — settle: keep waiting until no new `publishDiagnostics`
    /// arrive for [`DIAGNOSTICS_SETTLE_DURATION`], resetting the timer on each
    /// new publish. This avoids returning on rust-analyzer's initial clearing
    /// publish (empty diagnostics) before the real diagnostics arrive a few
    /// hundred milliseconds later.
    ///
    /// Both phases are bounded by the overall `timeout`.
    pub async fn wait_for_fresh_diagnostics(&self, version_before: u64, timeout: Duration) {
        tracing::debug!(
            version_before,
            timeout_ms = timeout.as_millis(),
            "wait_for_fresh_diagnostics: phase 1 — waiting for version to advance"
        );

        let deadline = tokio::time::Instant::now() + timeout;

        // ── Phase 1: wait for version to advance ──
        loop {
            let current_version = self.diagnostics_version.load(Ordering::Relaxed);
            if current_version != version_before {
                tracing::debug!(
                    version_before,
                    current_version,
                    "wait_for_fresh_diagnostics: version advanced, entering settle phase"
                );
                break;
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                tracing::warn!(
                    version_before,
                    current_version,
                    "wait_for_fresh_diagnostics: TIMED OUT in phase 1"
                );
                return;
            }

            tokio::select! {
                () = self.diagnostics_notify.notified() => {
                    tracing::debug!("wait_for_fresh_diagnostics: notified, checking version");
                }
                () = tokio::time::sleep(remaining) => {
                    let final_version = self.diagnostics_version.load(Ordering::Relaxed);
                    tracing::warn!(
                        version_before,
                        final_version,
                        "wait_for_fresh_diagnostics: TIMED OUT in phase 1 sleep"
                    );
                    return;
                }
            }
        }

        // ── Phase 2: settle — wait for DIAGNOSTICS_SETTLE_DURATION with no new publishes ──
        let mut last_version = self.diagnostics_version.load(Ordering::Relaxed);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                tracing::debug!(
                    last_version,
                    "wait_for_fresh_diagnostics: deadline reached during settle phase"
                );
                return;
            }

            let settle_wait = DIAGNOSTICS_SETTLE_DURATION.min(remaining);

            tokio::select! {
                () = self.diagnostics_notify.notified() => {
                    let new_version = self.diagnostics_version.load(Ordering::Relaxed);
                    if new_version != last_version {
                        tracing::debug!(
                            last_version,
                            new_version,
                            "wait_for_fresh_diagnostics: new publish during settle, resetting timer"
                        );
                        last_version = new_version;
                        // Loop again — restart the settle timer.
                    }
                }
                () = tokio::time::sleep(settle_wait) => {
                    let final_version = self.diagnostics_version.load(Ordering::Relaxed);
                    if final_version == last_version {
                        tracing::debug!(
                            final_version,
                            "wait_for_fresh_diagnostics: settled (no new publishes for {:?})",
                            settle_wait
                        );
                        return;
                    }
                    // Version changed just before we checked — go around again.
                    last_version = final_version;
                }
            }
        }
    }
}

/// Spawn a new LSP server process
fn spawn_lsp(root_path: &Path, command: &str, args: &[String]) -> DaemonResult<LspHandle> {
    let args_str: Vec<&str> = args.iter().map(std::string::String::as_str).collect();

    tracing::info!(
        %command,
        args = %args_str.join(" "),
        root = %root_path.display(),
        "Spawning LSP server process"
    );

    let mut process = Command::new(command)
        .args(&args_str)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| DaemonError::LspSpawnFailed(format!("{command}: {e}")))?;

    let stdin = process
        .stdin
        .take()
        .ok_or_else(|| DaemonError::LspSpawnFailed("Failed to capture stdin".into()))?;

    let stdout = process
        .stdout
        .take()
        .ok_or_else(|| DaemonError::LspSpawnFailed("Failed to capture stdout".into()))?;

    let (request_tx, request_rx) = mpsc::channel(100);
    let (notification_tx, notification_rx) = mpsc::channel(100);
    let uri_state = Arc::new(RwLock::new(HashMap::new()));
    let diagnostics_notify = Arc::new(Notify::new());
    let diagnostics_version = Arc::new(AtomicU64::new(0));
    let open_documents = Arc::new(RwLock::new(HashMap::new()));

    let handler_uri_state = Arc::clone(&uri_state);
    let handler_diagnostics_notify = Arc::clone(&diagnostics_notify);
    let handler_diagnostics_version = Arc::clone(&diagnostics_version);

    let (file_watch_tx, file_watch_rx) = mpsc::channel(64);
    let file_watcher = FileWatcherHandle::spawn(root_path.to_path_buf(), file_watch_tx);
    let handler = LspHandler {
        process,
        stdin,
        reader: BufReader::new(stdout),
        requests: request_rx,
        notifications: notification_rx,
        file_watcher_handle: file_watcher,
        file_watcher_rx: file_watch_rx,
        uri_state: handler_uri_state,
        diagnostics_notify: handler_diagnostics_notify,
        diagnostics_version: handler_diagnostics_version,
        root_path: root_path.to_path_buf(),
        next_id: 1,
        pending: HashMap::new(),
    };
    let task = tokio::spawn(handler.run());

    Ok(LspHandle {
        request_tx,
        notification_tx,
        uri_state,
        diagnostics_notify,
        diagnostics_version,
        open_documents,
        _task: task,
    })
}

fn hash_content(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Owns all LSP handler state and runs the event loop.
struct LspHandler {
    process: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    requests: mpsc::Receiver<LspRequestEnvelope>,
    notifications: mpsc::Receiver<LspNotification>,
    file_watcher_handle: FileWatcherHandle,
    file_watcher_rx: mpsc::Receiver<FileWatcherBatch>,
    uri_state: Arc<RwLock<HashMap<Uri, Option<PublishDiagnosticsParams>>>>,
    diagnostics_notify: Arc<Notify>,
    diagnostics_version: Arc<AtomicU64>,
    root_path: PathBuf,
    next_id: i64,
    pending: HashMap<i64, oneshot::Sender<Result<Value, LspErrorResponse>>>,
}

impl LspHandler {
    /// Run the LSP handler: initialize, process messages, clean up.
    async fn run(mut self) {
        tracing::info!(
            root = %self.root_path.display(),
            "LspHandler: starting initialization"
        );

        if let Err(e) = self.initialize().await {
            tracing::error!(%e, "LspHandler: failed to send initialize request");
            return;
        }

        if !self.wait_for_init_response().await {
            tracing::error!("LspHandler: initialization response failed");
            return;
        }

        let _ =
            send_notification(&mut self.stdin, Initialized::METHOD, serde_json::json!({})).await;

        tracing::info!("LspHandler: initialization complete, entering main loop");
        self.run_main_loop().await;
        tracing::info!("LspHandler: main loop exited, cleaning up");
        self.cleanup_pending();
    }

    /// Wait for the initialize response, handling any server messages that arrive first.
    /// Returns `true` if initialization succeeded, `false` if the connection was lost.
    async fn wait_for_init_response(&mut self) -> bool {
        loop {
            match read_lsp_message(&mut self.reader).await {
                Ok(Some(msg)) => {
                    if msg.get("id").and_then(serde_json::Value::as_i64) == Some(1) {
                        tracing::debug!("LSP initialized");
                        return true;
                    }
                    self.handle_lsp_message(msg).await;
                }
                Ok(None) => {
                    tracing::error!("LSP closed during initialization");
                    return false;
                }
                Err(e) => {
                    tracing::error!("Error reading LSP message: {}", e);
                    return false;
                }
            }
        }
    }

    /// Main event loop: multiplex LSP stdout, outgoing requests, notifications,
    /// file-watch events, and process exit.
    async fn run_main_loop(&mut self) {
        loop {
            tokio::select! {
                msg = read_lsp_message(&mut self.reader) => {
                    match msg {
                        Ok(Some(msg)) => self.handle_lsp_message(msg).await,
                        Ok(None) => {
                            tracing::info!("LSP server closed connection");
                            break;
                        }
                        Err(e) => {
                            tracing::warn!("Error reading LSP message: {}", e);
                        }
                    }
                }

                Some(envelope) = self.requests.recv() => {
                    self.handle_outgoing_request(envelope).await;
                }

                Some(notification) = self.notifications.recv() => {
                    if let Err(e) = self.send_client_notification(notification).await {
                        tracing::warn!("Failed to forward notification: {}", e);
                    }
                }

                Some(batch) = self.file_watcher_rx.recv() => {
                    let forwarded_count = batch.forwarded_changes.len();
                    let discovered_count = batch.discovered_uris.len();

                    {
                        let mut state = self.uri_state.write().await;
                        for uri in &batch.discovered_uris {
                            state.entry(uri.clone()).or_insert(None);
                        }
                        for change in &batch.forwarded_changes {
                            state.entry(change.uri.clone()).or_insert(None);
                        }
                    }

                    if forwarded_count == 0 {
                        tracing::debug!(
                            discovered_uris = discovered_count,
                            "File watcher: discovered URIs with no forwarded didChangeWatchedFiles"
                        );
                        continue;
                    }

                    tracing::info!(
                        forwarded_changes = forwarded_count,
                        discovered_uris = discovered_count,
                        files = %batch.forwarded_changes.iter().map(|c| c.uri.as_str()).collect::<Vec<_>>().join(", "),
                        "File watcher: forwarding didChangeWatchedFiles to LSP"
                    );
                    let params = DidChangeWatchedFilesParams {
                        changes: batch.forwarded_changes,
                    };
                    if let Ok(value) = serde_json::to_value(&params)
                        && let Err(e) = send_notification(&mut self.stdin, DidChangeWatchedFiles::METHOD, value).await
                    {
                        tracing::warn!(%e, "Failed to send didChangeWatchedFiles");
                    }
                }

                _ = self.process.wait() => {
                    tracing::info!("LSP process exited");
                    break;
                }
            }
        }
    }

    /// Send an outgoing request to the LSP server, tracking it in `pending`.
    async fn handle_outgoing_request(&mut self, envelope: LspRequestEnvelope) {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, envelope.response_tx);

        tracing::debug!(id, method = %envelope.method, "Sending LSP request");

        if let Err(e) = self
            .send_request(id, &envelope.method, envelope.params)
            .await
        {
            tracing::error!(id, %e, "Failed to send LSP request");
            if let Some(tx) = self.pending.remove(&id) {
                let _ = tx.send(Err(LspErrorResponse {
                    code: -1,
                    message: e.to_string(),
                }));
            }
        }
    }

    /// Drain all pending requests with an error on shutdown.
    fn cleanup_pending(&mut self) {
        for (_, tx) in self.pending.drain() {
            let _ = tx.send(Err(LspErrorResponse {
                code: -1,
                message: "LSP server closed".into(),
            }));
        }
    }

    /// Handle an incoming LSP message.
    ///
    /// Messages from the LSP server fall into three categories:
    /// - **Server request** (has `id` AND `method`): e.g. `client/registerCapability`
    /// - **Response** (has `id`, no `method`): reply to a request we sent
    /// - **Notification** (has `method`, no `id`): e.g. `textDocument/publishDiagnostics`
    async fn handle_lsp_message(&mut self, msg: Value) {
        let has_id = msg.get("id").is_some();
        let method = msg.get("method").and_then(|v| v.as_str());

        match (has_id, method) {
            // Server-to-client request: has both `id` and `method`
            (true, Some(method_str)) => {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                let params = msg.get("params").cloned().unwrap_or(Value::Null);
                tracing::debug!("Received server request: {method_str}");

                match method_str {
                    RegisterCapability::METHOD => {
                        self.handle_register_capability(&id, &params).await;
                    }
                    UnregisterCapability::METHOD => {
                        self.handle_unregister_capability(&id, &params).await;
                    }
                    WorkDoneProgressCreate::METHOD => {
                        let _ = self.send_ok_response(&id).await;
                    }
                    _ => {
                        tracing::debug!("Unhandled server request: {method_str}");
                        let response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32601,
                                "message": format!("Method not found: {method_str}")
                            }
                        });
                        let _ = write_lsp_message(&mut self.stdin, &response).await;
                    }
                }
            }

            // Response to our request: has `id`, no `method`
            (true, None) => {
                if let Some(id) = msg.get("id").and_then(serde_json::Value::as_i64)
                    && let Some(tx) = self.pending.remove(&id)
                {
                    let result = if let Some(error) = msg.get("error") {
                        let code = error
                            .get("code")
                            .and_then(serde_json::Value::as_i64)
                            .and_then(|c| i32::try_from(c).ok())
                            .unwrap_or(-1);
                        let message = error
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error")
                            .to_string();
                        Err(LspErrorResponse { code, message })
                    } else {
                        Ok(msg.get("result").cloned().unwrap_or(Value::Null))
                    };
                    let _ = tx.send(result);
                }
            }

            // Server notification: has `method`, no `id`
            (false, Some(method_str)) => {
                if method_str == PublishDiagnostics::METHOD {
                    let params = msg.get("params").cloned().unwrap_or(Value::Null);
                    if let Ok(diag_params) =
                        serde_json::from_value::<PublishDiagnosticsParams>(params)
                    {
                        let error_count = diag_params
                            .diagnostics
                            .iter()
                            .filter(|d| d.severity == Some(lsp_types::DiagnosticSeverity::ERROR))
                            .count();
                        let warning_count = diag_params
                            .diagnostics
                            .iter()
                            .filter(|d| d.severity == Some(lsp_types::DiagnosticSeverity::WARNING))
                            .count();

                        let new_version = self.diagnostics_version.load(Ordering::Relaxed) + 1;

                        tracing::info!(
                            uri = %diag_params.uri.as_str(),
                            total = diag_params.diagnostics.len(),
                            errors = error_count,
                            warnings = warning_count,
                            new_version,
                            "publishDiagnostics received"
                        );

                        self.uri_state
                            .write()
                            .await
                            .insert(diag_params.uri.clone(), Some(diag_params));
                        self.diagnostics_version.fetch_add(1, Ordering::Relaxed);
                        self.diagnostics_notify.notify_waiters();
                    }
                } else {
                    tracing::debug!(method = method_str, "Server notification (non-diagnostic)");
                }
            }

            // Invalid message: no `id` and no `method`
            (false, None) => {
                tracing::debug!("Received LSP message with neither id nor method");
            }
        }
    }

    /// Handle `client/registerCapability` — register file watchers for
    /// `workspace/didChangeWatchedFiles` registrations and respond with success.
    async fn handle_register_capability(&mut self, id: &Value, params: &Value) {
        if let Ok(reg_params) = serde_json::from_value::<RegistrationParams>(params.clone()) {
            for reg in &reg_params.registrations {
                if reg.method == DidChangeWatchedFiles::METHOD
                    && let Some(opts) = &reg.register_options
                    && let Ok(fs_watchers) = parse_file_system_watchers(opts)
                {
                    tracing::debug!(
                        "Registering {} file watchers for {}",
                        fs_watchers.len(),
                        reg.id
                    );
                    self.file_watcher_handle
                        .register_watchers(reg.id.clone(), fs_watchers);
                }
            }
        }

        let _ = self.send_ok_response(id).await;
    }

    /// Handle `client/unregisterCapability` — unregister file watchers and respond with success.
    async fn handle_unregister_capability(&mut self, id: &Value, params: &Value) {
        if let Ok(unreg_params) =
            serde_json::from_value::<lsp_types::UnregistrationParams>(params.clone())
        {
            for unreg in &unreg_params.unregisterations {
                if unreg.method == DidChangeWatchedFiles::METHOD {
                    self.file_watcher_handle.unregister(unreg.id.clone());
                }
            }
        }

        let _ = self.send_ok_response(id).await;
    }

    /// Send the `initialize` request to the LSP server.
    async fn initialize(&mut self) -> std::io::Result<()> {
        let root_uri = path_to_uri(&self.root_path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let capabilities = ClientCapabilities {
            general: Some(GeneralClientCapabilities::default()),
            text_document: Some(TextDocumentClientCapabilities {
                publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                    related_information: Some(true),
                    ..Default::default()
                }),
                definition: Some(GotoCapability {
                    dynamic_registration: Some(false),
                    link_support: Some(true),
                }),
                implementation: Some(GotoCapability {
                    dynamic_registration: Some(false),
                    link_support: Some(true),
                }),
                references: Some(DynamicRegistrationClientCapabilities {
                    dynamic_registration: Some(false),
                }),
                hover: Some(HoverClientCapabilities {
                    dynamic_registration: Some(false),
                    content_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
                }),
                document_symbol: Some(DocumentSymbolClientCapabilities {
                    hierarchical_document_symbol_support: Some(true),
                    ..Default::default()
                }),
                call_hierarchy: Some(CallHierarchyClientCapabilities {
                    dynamic_registration: Some(false),
                }),
                ..Default::default()
            }),
            workspace: Some(WorkspaceClientCapabilities {
                did_change_watched_files: Some(DidChangeWatchedFilesClientCapabilities {
                    dynamic_registration: Some(true),
                    relative_pattern_support: Some(false),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let params = InitializeParams {
            process_id: Some(std::process::id()),
            #[allow(deprecated)]
            root_uri: Some(root_uri),
            capabilities,
            ..Default::default()
        };

        let id = self.next_id;
        self.next_id += 1;
        self.send_request(
            id,
            Initialize::METHOD,
            serde_json::to_value(&params).unwrap(),
        )
        .await
    }

    /// Send a `{ jsonrpc: "2.0", id, result: null }` success response.
    async fn send_ok_response(&mut self, id: &Value) -> std::io::Result<()> {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": null
        });
        write_lsp_message(&mut self.stdin, &response).await
    }

    /// Send an LSP request.
    async fn send_request(&mut self, id: i64, method: &str, params: Value) -> std::io::Result<()> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        write_lsp_message(&mut self.stdin, &msg).await
    }

    /// Send a client notification to the LSP server.
    async fn send_client_notification(
        &mut self,
        notification: LspNotification,
    ) -> std::io::Result<()> {
        tracing::debug!(method = %notification.method, "Forwarding client notification to LSP");
        send_notification(&mut self.stdin, &notification.method, notification.params).await
    }
}

/// Parse `FileSystemWatcher`s from the `registerOptions` JSON of a
/// `workspace/didChangeWatchedFiles` registration.
fn parse_file_system_watchers(
    opts: &Value,
) -> Result<Vec<lsp_types::FileSystemWatcher>, serde_json::Error> {
    #[derive(serde::Deserialize)]
    struct WatcherOpts {
        watchers: Vec<lsp_types::FileSystemWatcher>,
    }

    let parsed: WatcherOpts = serde_json::from_value(opts.clone())?;
    Ok(parsed.watchers)
}

/// Send an LSP notification
async fn send_notification(
    stdin: &mut ChildStdin,
    method: &str,
    params: Value,
) -> std::io::Result<()> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params
    });
    write_lsp_message(stdin, &msg).await
}

/// Read an LSP message from stdout
async fn read_lsp_message(reader: &mut BufReader<ChildStdout>) -> std::io::Result<Option<Value>> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut header = String::new();
        let bytes = reader.read_line(&mut header).await?;

        if bytes == 0 {
            return Ok(None);
        }

        let header = header.trim();

        if header.is_empty() {
            break;
        }

        if let Some(value) = header.strip_prefix("Content-Length: ") {
            content_length = value.parse().ok();
        }
    }

    let content_length = content_length.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "Missing Content-Length")
    })?;

    let mut buf = vec![0u8; content_length];
    reader.read_exact(&mut buf).await?;

    serde_json::from_slice(&buf)
        .map(Some)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Write an LSP message to stdin
async fn write_lsp_message(stdin: &mut ChildStdin, msg: &Value) -> std::io::Result<()> {
    let content = serde_json::to_string(msg)?;
    let header = format!("Content-Length: {}\r\n\r\n", content.len());
    stdin.write_all(header.as_bytes()).await?;
    stdin.write_all(content.as_bytes()).await?;
    stdin.flush().await
}

#[cfg(test)]
#[allow(clippy::mutable_key_type)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::Path;

    fn test_uri(path: &str) -> Uri {
        path_to_uri(Path::new(path)).expect("valid test URI path")
    }

    fn test_diagnostics(uri: Uri) -> PublishDiagnosticsParams {
        PublishDiagnosticsParams {
            uri,
            diagnostics: Vec::new(),
            version: None,
        }
    }

    fn test_handle_with_known_uris(known_uris: &[Uri]) -> LspHandle {
        let (request_tx, _request_rx) = mpsc::channel(1);
        let (notification_tx, _notification_rx) = mpsc::channel(1);

        let mut uri_state = HashMap::new();
        for uri in known_uris {
            uri_state.insert(uri.clone(), Some(test_diagnostics(uri.clone())));
        }

        LspHandle {
            request_tx,
            notification_tx,
            uri_state: Arc::new(RwLock::new(uri_state)),
            open_documents: Arc::new(RwLock::new(HashMap::new())),
            diagnostics_notify: Arc::new(Notify::new()),
            diagnostics_version: Arc::new(AtomicU64::new(0)),
            _task: tokio::spawn(async {}),
        }
    }

    #[tokio::test]
    async fn test_wait_for_fresh_diagnostics_waits_for_late_followup_publish() {
        let handle = test_handle_with_known_uris(&[]);
        let diagnostics_notify = Arc::clone(&handle.diagnostics_notify);
        let diagnostics_version = Arc::clone(&handle.diagnostics_version);

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            diagnostics_version.fetch_add(1, Ordering::Relaxed);
            diagnostics_notify.notify_waiters();

            // Simulate rust-analyzer's common pattern: an initial clearing publish,
            // then the real diagnostics a few hundred milliseconds later.
            tokio::time::sleep(Duration::from_millis(400)).await;
            diagnostics_version.fetch_add(1, Ordering::Relaxed);
            diagnostics_notify.notify_waiters();
        });

        handle
            .wait_for_fresh_diagnostics(0, Duration::from_secs(2))
            .await;

        assert_eq!(
            handle.diagnostics_version.load(Ordering::Relaxed),
            2,
            "wait_for_fresh_diagnostics returned before the late follow-up publish arrived"
        );
    }

    #[tokio::test]
    async fn test_forget_known_uris_prunes_known_and_diagnostics_cache() {
        let uri_a = test_uri("/tmp/project/a.rs");
        let uri_b = test_uri("/tmp/project/b.rs");
        let handle = test_handle_with_known_uris(&[uri_a.clone(), uri_b.clone()]);

        handle.forget_known_uris(std::slice::from_ref(&uri_a)).await;

        let state = handle.uri_state.read().await;
        assert!(!state.contains_key(&uri_a));
        assert!(state.contains_key(&uri_b));
    }

    #[tokio::test]
    async fn test_forget_known_uris_removes_future_all_files_sync_candidates() {
        let removed_uri = test_uri("/tmp/project/stale.rs");
        let retained_uri = test_uri("/tmp/project/stable.rs");
        let handle = test_handle_with_known_uris(&[removed_uri.clone(), retained_uri.clone()]);

        handle
            .forget_known_uris(std::slice::from_ref(&removed_uri))
            .await;

        let cached_uris = handle.cached_uris().await;
        assert!(!cached_uris.contains(&removed_uri));
        assert!(cached_uris.contains(&retained_uri));
    }
}
