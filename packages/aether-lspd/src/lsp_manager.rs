use crate::error::{DaemonError, DaemonResult};
use crate::protocol::LspNotification;
use lsp_types::{
    CallHierarchyClientCapabilities, CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams,
    CallHierarchyItem, CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams, ClientCapabilities, DocumentSymbolClientCapabilities,
    DocumentSymbolParams, DocumentSymbolResponse, DynamicRegistrationClientCapabilities,
    GeneralClientCapabilities, GotoCapability, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverClientCapabilities, HoverParams, InitializeParams, Location, MarkupKind,
    PublishDiagnosticsClientCapabilities, PublishDiagnosticsParams, ReferenceParams,
    SymbolInformation, TextDocumentClientCapabilities, Uri, WorkspaceSymbolParams,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::task::JoinHandle;

/// Key for identifying an LSP instance
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct LspKey {
    pub workspace_root: PathBuf,
    pub language: String,
}

/// Trait for LSP operations - eliminates the need for separate Op/Result/Kind enums
pub trait LspOperation: Send + Sync {
    type Response: DeserializeOwned + Send + 'static;

    fn method(&self) -> &'static str;
    fn params(&self) -> Value;
    fn default_response() -> Self::Response;
}

/// Wrapper for GotoImplementation (uses same params as GotoDefinition)
pub struct GotoImplementation(pub GotoDefinitionParams);

impl LspOperation for GotoDefinitionParams {
    type Response = GotoDefinitionResponse;
    fn method(&self) -> &'static str {
        "textDocument/definition"
    }
    fn params(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
    fn default_response() -> Self::Response {
        GotoDefinitionResponse::Array(vec![])
    }
}

impl LspOperation for GotoImplementation {
    type Response = GotoDefinitionResponse;
    fn method(&self) -> &'static str {
        "textDocument/implementation"
    }
    fn params(&self) -> Value {
        serde_json::to_value(&self.0).unwrap()
    }
    fn default_response() -> Self::Response {
        GotoDefinitionResponse::Array(vec![])
    }
}

impl LspOperation for ReferenceParams {
    type Response = Vec<Location>;
    fn method(&self) -> &'static str {
        "textDocument/references"
    }
    fn params(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
    fn default_response() -> Self::Response {
        vec![]
    }
}

impl LspOperation for HoverParams {
    type Response = Option<Hover>;
    fn method(&self) -> &'static str {
        "textDocument/hover"
    }
    fn params(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
    fn default_response() -> Self::Response {
        None
    }
}

impl LspOperation for WorkspaceSymbolParams {
    type Response = Vec<SymbolInformation>;
    fn method(&self) -> &'static str {
        "workspace/symbol"
    }
    fn params(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
    fn default_response() -> Self::Response {
        vec![]
    }
}

impl LspOperation for DocumentSymbolParams {
    type Response = DocumentSymbolResponse;
    fn method(&self) -> &'static str {
        "textDocument/documentSymbol"
    }
    fn params(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
    fn default_response() -> Self::Response {
        DocumentSymbolResponse::Flat(vec![])
    }
}

impl LspOperation for CallHierarchyPrepareParams {
    type Response = Vec<CallHierarchyItem>;
    fn method(&self) -> &'static str {
        "textDocument/prepareCallHierarchy"
    }
    fn params(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
    fn default_response() -> Self::Response {
        vec![]
    }
}

impl LspOperation for CallHierarchyIncomingCallsParams {
    type Response = Vec<CallHierarchyIncomingCall>;
    fn method(&self) -> &'static str {
        "callHierarchy/incomingCalls"
    }
    fn params(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
    fn default_response() -> Self::Response {
        vec![]
    }
}

impl LspOperation for CallHierarchyOutgoingCallsParams {
    type Response = Vec<CallHierarchyOutgoingCall>;
    fn method(&self) -> &'static str {
        "callHierarchy/outgoingCalls"
    }
    fn params(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
    fn default_response() -> Self::Response {
        vec![]
    }
}

/// Manager for LSP server instances (simplified from actor pattern)
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
    /// Background task handle
    _task: JoinHandle<()>,
}

/// Envelope for an LSP request with response channel (simplified: uses Value)
struct LspRequestEnvelope {
    method: &'static str,
    params: Value,
    response_tx: oneshot::Sender<Result<Value, LspErrorInfo>>,
}

/// Error information from LSP
#[derive(Debug, Clone)]
pub struct LspErrorInfo {
    pub code: i32,
    pub message: String,
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

        let handle = Arc::new(spawn_lsp(&key.workspace_root, command, args).await?);
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
    /// Send a request to the LSP and wait for response
    pub async fn request<O: LspOperation>(&self, op: O) -> Result<O::Response, LspErrorInfo> {
        let (response_tx, response_rx) = oneshot::channel();
        let envelope = LspRequestEnvelope {
            method: op.method(),
            params: op.params(),
            response_tx,
        };

        self.request_tx
            .send(envelope)
            .await
            .map_err(|_| LspErrorInfo {
                code: -1,
                message: "LSP handler closed".into(),
            })?;

        let result = response_rx.await.map_err(|_| LspErrorInfo {
            code: -1,
            message: "Response channel closed".into(),
        })?;

        // Parse the Value response into the expected type
        result.and_then(|value| {
            if value.is_null() {
                Ok(O::default_response())
            } else {
                serde_json::from_value(value).map_err(|e| LspErrorInfo {
                    code: -1,
                    message: format!("Parse error: {}", e),
                })
            }
        })
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
}

/// Spawn a new LSP server process
async fn spawn_lsp(root_path: &Path, command: &str, args: &[String]) -> DaemonResult<LspHandle> {
    let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let mut process = Command::new(command)
        .args(&args_str)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| DaemonError::LspSpawnFailed(format!("{}: {}", command, e)))?;

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

    let handler_diagnostics_cache = Arc::clone(&diagnostics_cache);
    let handler_root_path = root_path.to_path_buf();

    let task = tokio::spawn(async move {
        run_lsp_handler(
            process,
            stdin,
            stdout,
            request_rx,
            notification_rx,
            handler_diagnostics_cache,
            handler_root_path,
        )
        .await;
    });

    Ok(LspHandle {
        request_tx,
        notification_tx,
        diagnostics_cache,
        _task: task,
    })
}

/// Run the LSP handler loop
async fn run_lsp_handler(
    mut process: Child,
    mut stdin: ChildStdin,
    stdout: ChildStdout,
    mut request_rx: mpsc::Receiver<LspRequestEnvelope>,
    mut notification_rx: mpsc::Receiver<LspNotification>,
    diagnostics_cache: Arc<RwLock<HashMap<Uri, PublishDiagnosticsParams>>>,
    root_path: PathBuf,
) {
    let mut reader = BufReader::new(stdout);
    let mut next_id: i64 = 1;
    let mut pending: HashMap<i64, oneshot::Sender<Result<Value, LspErrorInfo>>> = HashMap::new();

    if let Err(e) = initialize_lsp(&mut stdin, &mut next_id, &root_path).await {
        tracing::error!("Failed to initialize LSP: {}", e);
        return;
    }

    loop {
        match read_lsp_message(&mut reader).await {
            Ok(Some(msg)) => {
                if msg.get("id").and_then(|v| v.as_i64()) == Some(1) {
                    tracing::debug!("LSP initialized");
                    break;
                }
            }
            Ok(None) => {
                tracing::error!("LSP closed during initialization");
                return;
            }
            Err(e) => {
                tracing::error!("Error reading LSP message: {}", e);
                return;
            }
        }
    }

    let _ = send_notification(&mut stdin, "initialized", serde_json::json!({})).await;

    loop {
        tokio::select! {
            msg = read_lsp_message(&mut reader) => {
                match msg {
                    Ok(Some(msg)) => {
                        handle_lsp_message(msg, &mut pending, &diagnostics_cache).await;
                    }
                    Ok(None) => {
                        tracing::info!("LSP server closed connection");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Error reading LSP message: {}", e);
                    }
                }
            }

            Some(envelope) = request_rx.recv() => {
                let id = next_id;
                next_id += 1;
                pending.insert(id, envelope.response_tx);

                if let Err(e) = send_request(&mut stdin, id, envelope.method, envelope.params).await {
                    tracing::error!("Failed to send LSP request: {}", e);
                    if let Some(tx) = pending.remove(&id) {
                        let _ = tx.send(Err(LspErrorInfo {
                            code: -1,
                            message: e.to_string(),
                        }));
                    }
                }
            }

            Some(notification) = notification_rx.recv() => {
                if let Err(e) = send_client_notification(&mut stdin, &notification).await {
                    tracing::warn!("Failed to forward notification: {}", e);
                }
            }

            _ = process.wait() => {
                tracing::info!("LSP process exited");
                break;
            }
        }
    }

    // Clean up pending requests on shutdown
    for (_, tx) in pending {
        let _ = tx.send(Err(LspErrorInfo {
            code: -1,
            message: "LSP server closed".into(),
        }));
    }
}

/// Handle an incoming LSP message
async fn handle_lsp_message(
    msg: Value,
    pending: &mut HashMap<i64, oneshot::Sender<Result<Value, LspErrorInfo>>>,
    diagnostics_cache: &RwLock<HashMap<Uri, PublishDiagnosticsParams>>,
) {
    // Handle response messages
    if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
        if let Some(tx) = pending.remove(&id) {
            let result = if let Some(error) = msg.get("error") {
                let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
                let message = error
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                Err(LspErrorInfo { code, message })
            } else {
                Ok(msg.get("result").cloned().unwrap_or(Value::Null))
            };
            let _ = tx.send(result);
        }
        return;
    }

    // Handle notification messages - cache diagnostics
    if let Some(method) = msg.get("method").and_then(|v| v.as_str())
        && method == "textDocument/publishDiagnostics"
    {
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        if let Ok(diag_params) = serde_json::from_value::<PublishDiagnosticsParams>(params) {
            let mut cache = diagnostics_cache.write().await;
            cache.insert(diag_params.uri.clone(), diag_params);
        }
    }
}

/// Initialize the LSP server
async fn initialize_lsp(
    stdin: &mut ChildStdin,
    next_id: &mut i64,
    root_path: &Path,
) -> std::io::Result<()> {
    let root_uri = path_to_uri(root_path);

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
        ..Default::default()
    };

    let params = InitializeParams {
        process_id: Some(std::process::id()),
        #[allow(deprecated)]
        root_uri: Some(root_uri),
        capabilities,
        ..Default::default()
    };

    let id = *next_id;
    *next_id += 1;
    send_request(
        stdin,
        id,
        "initialize",
        serde_json::to_value(&params).unwrap(),
    )
    .await
}

/// Convert path to URI
fn path_to_uri(path: &Path) -> Uri {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };

    #[cfg(windows)]
    let uri_str = {
        let path_str = absolute.to_string_lossy().replace('\\', "/");
        format!("file:///{}", path_str)
    };

    #[cfg(not(windows))]
    let uri_str = format!("file://{}", absolute.display());

    uri_str
        .parse()
        .unwrap_or_else(|_| "file:///".parse().unwrap())
}

/// Send an LSP request
async fn send_request(
    stdin: &mut ChildStdin,
    id: i64,
    method: &str,
    params: Value,
) -> std::io::Result<()> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    });
    write_lsp_message(stdin, &msg).await
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

/// Send a client notification to the LSP server
async fn send_client_notification(
    stdin: &mut ChildStdin,
    notification: &LspNotification,
) -> std::io::Result<()> {
    let (method, params) = match notification {
        LspNotification::Opened(p) => ("textDocument/didOpen", serde_json::to_value(p).unwrap()),
        LspNotification::Changed(p) => ("textDocument/didChange", serde_json::to_value(p).unwrap()),
        LspNotification::Saved(p) => ("textDocument/didSave", serde_json::to_value(p).unwrap()),
        LspNotification::Closed(p) => ("textDocument/didClose", serde_json::to_value(p).unwrap()),
    };
    send_notification(stdin, method, params).await
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
