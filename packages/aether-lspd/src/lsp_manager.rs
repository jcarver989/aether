use crate::error::{DaemonError, DaemonResult};
use crate::file_watcher::FileWatcherHandle;
use crate::language_id::LanguageId;
use crate::protocol::{LspErrorResponse, LspNotification};
use crate::uri::{path_to_uri, uri_to_path};
use lsp_types::notification::{
    DidChangeWatchedFiles, DidOpenTextDocument, Initialized, Notification, PublishDiagnostics,
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
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::fs::read_to_string;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::task::JoinHandle;

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
    /// Cached diagnostics keyed by file URI
    diagnostics_cache: Arc<RwLock<HashMap<Uri, PublishDiagnosticsParams>>>,
    /// Documents that have been opened with didOpen
    open_documents: Arc<RwLock<HashSet<Uri>>>,
    /// Background task handle
    _task: JoinHandle<()>,
}

/// Envelope for an LSP request with response channel
struct LspRequestEnvelope {
    method: String,
    params: Value,
    response_tx: oneshot::Sender<Result<Value, LspErrorResponse>>,
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
    pub async fn request_raw(&self, method: &str, params: Value) -> Result<Value, LspErrorResponse> {
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
        let cache = self.diagnostics_cache.read().await;
        match uri {
            Some(u) => cache.get(u).cloned().into_iter().collect(),
            None => cache.values().cloned().collect(),
        }
    }

    /// Send a notification to the LSP server (fire-and-forget)
    pub async fn send_notification(&self, notification: LspNotification) {
        if let Err(e) = self.notification_tx.send(notification).await {
            tracing::warn!("Failed to send notification: {}", e);
        }
    }

    /// Ensure a document is open before sending an LSP request that targets it.
    ///
    /// If the document hasn't been opened yet, reads it from disk and sends
    /// a `didOpen` notification. This allows agents to skip explicit didOpen
    /// management — the daemon handles it transparently.
    pub async fn ensure_document_open(&self, uri: &Uri) {
        if self.open_documents.read().await.contains(uri) {
            return;
        }

        let file_path = uri_to_path(uri);
        let content = match read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("Could not read file for auto-open {}: {}", file_path, e);
                return;
            }
        };

        let mut open_docs = self.open_documents.write().await;
        if open_docs.contains(uri) {
            return;
        }

        let language_id = LanguageId::from_path(Path::new(&file_path));
        let params = lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: uri.clone(),
                language_id: language_id.as_str().to_string(),
                version: 1,
                text: content,
            },
        };

        let notification = LspNotification {
            method: DidOpenTextDocument::METHOD.to_string(),
            params: serde_json::to_value(&params).unwrap(),
        };
        self.send_notification(notification).await;
        open_docs.insert(uri.clone());
    }
}

/// Spawn a new LSP server process
fn spawn_lsp(root_path: &Path, command: &str, args: &[String]) -> DaemonResult<LspHandle> {
    let args_str: Vec<&str> = args.iter().map(std::string::String::as_str).collect();

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
    let diagnostics_cache = Arc::new(RwLock::new(HashMap::new()));
    let open_documents = Arc::new(RwLock::new(HashSet::new()));

    let handler_diagnostics_cache = Arc::clone(&diagnostics_cache);

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
        diagnostics_cache: handler_diagnostics_cache,
        root_path: root_path.to_path_buf(),
        next_id: 1,
        pending: HashMap::new(),
    };
    let task = tokio::spawn(handler.run());

    Ok(LspHandle {
        request_tx,
        notification_tx,
        diagnostics_cache,
        open_documents,
        _task: task,
    })
}

/// Owns all LSP handler state and runs the event loop.
struct LspHandler {
    process: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    requests: mpsc::Receiver<LspRequestEnvelope>,
    notifications: mpsc::Receiver<LspNotification>,
    file_watcher_handle: FileWatcherHandle,
    file_watcher_rx: mpsc::Receiver<DidChangeWatchedFilesParams>,
    diagnostics_cache: Arc<RwLock<HashMap<Uri, PublishDiagnosticsParams>>>,
    root_path: PathBuf,
    next_id: i64,
    pending: HashMap<i64, oneshot::Sender<Result<Value, LspErrorResponse>>>,
}

impl LspHandler {
    /// Run the LSP handler: initialize, process messages, clean up.
    async fn run(mut self) {
        if let Err(e) = self.initialize().await {
            tracing::error!("Failed to initialize LSP: {}", e);
            return;
        }

        if !self.wait_for_init_response().await {
            return;
        }

        let _ =
            send_notification(&mut self.stdin, Initialized::METHOD, serde_json::json!({})).await;

        self.run_main_loop().await;
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

                Some(params) = self.file_watcher_rx.recv() => {
                    if let Ok(value) = serde_json::to_value(&params)
                        && let Err(e) = send_notification(&mut self.stdin, DidChangeWatchedFiles::METHOD, value).await
                    {
                        tracing::warn!("Failed to send didChangeWatchedFiles: {}", e);
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

        if let Err(e) = self
            .send_request(id, &envelope.method, envelope.params)
            .await
        {
            tracing::error!("Failed to send LSP request: {}", e);
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
                        let mut cache = self.diagnostics_cache.write().await;
                        cache.insert(diag_params.uri.clone(), diag_params);
                    }
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
        let root_uri = path_to_uri(&self.root_path).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
        })?;

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
