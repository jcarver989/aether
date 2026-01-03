use crate::error::{DaemonError, DaemonResult};
use crate::protocol::{LspNotification, ServerNotification};
use lsp_types::{
    CallHierarchyClientCapabilities, CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams,
    CallHierarchyItem, CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams, ClientCapabilities, DocumentSymbolClientCapabilities,
    DocumentSymbolParams, DocumentSymbolResponse, DynamicRegistrationClientCapabilities,
    GeneralClientCapabilities, GotoCapability, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverClientCapabilities, HoverParams, InitializeParams, Location, MarkupKind, ProgressParams,
    PublishDiagnosticsClientCapabilities, PublishDiagnosticsParams, ReferenceParams,
    SymbolInformation, TextDocumentClientCapabilities, Uri, WindowClientCapabilities,
    WorkspaceSymbolParams,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
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

/// Messages that can be sent to the LSP manager actor
enum LspManagerRequest {
    GetOrSpawn {
        key: LspKey,
        command: String,
        args: Vec<String>,
        response_tx: oneshot::Sender<DaemonResult<Arc<LspHandle>>>,
    },
    Shutdown {
        response_tx: oneshot::Sender<()>,
    },
}

/// Actor that manages active LSP server processes
struct LspManagerActor {
    /// Active LSP instances keyed by (workspace, language)
    lsps: HashMap<LspKey, Arc<LspHandle>>,
    /// Request receiver
    request_rx: mpsc::Receiver<LspManagerRequest>,
}

/// Handle to communicate with the LSP manager actor
#[derive(Clone)]
pub struct LspManagerHandle {
    request_tx: mpsc::Sender<LspManagerRequest>,
}

/// Handle to an active LSP server
pub struct LspHandle {
    /// Request sender
    request_tx: mpsc::Sender<LspRequestEnvelope>,
    /// Notification sender (fire-and-forget)
    notification_tx: mpsc::Sender<LspNotification>,
    /// Cached diagnostics keyed by file URI
    diagnostics_cache: Arc<RwLock<HashMap<Uri, PublishDiagnosticsParams>>>,
    /// Subscribers for server notifications (diagnostics, progress).
    /// Used by `subscribe()` for push-based notification delivery.
    #[allow(dead_code)]
    subscribers: Arc<RwLock<Vec<mpsc::Sender<ServerNotification>>>>,
    /// Background task handle
    _task: JoinHandle<()>,
}

/// Envelope for an LSP request with response channel
struct LspRequestEnvelope {
    request: LspOp,
    response_tx: oneshot::Sender<LspResult>,
}

/// Operation to perform on the LSP server
#[derive(Debug)]
pub enum LspOp {
    GotoDefinition(GotoDefinitionParams),
    GotoImplementation(GotoDefinitionParams),
    FindReferences(ReferenceParams),
    Hover(HoverParams),
    WorkspaceSymbol(WorkspaceSymbolParams),
    DocumentSymbol(DocumentSymbolParams),
    PrepareCallHierarchy(CallHierarchyPrepareParams),
    IncomingCalls(CallHierarchyIncomingCallsParams),
    OutgoingCalls(CallHierarchyOutgoingCallsParams),
}

/// Result of an LSP request
#[derive(Debug)]
pub enum LspResult {
    GotoDefinition(std::result::Result<GotoDefinitionResponse, LspErrorInfo>),
    GotoImplementation(std::result::Result<GotoDefinitionResponse, LspErrorInfo>),
    FindReferences(std::result::Result<Vec<Location>, LspErrorInfo>),
    Hover(std::result::Result<Option<Hover>, LspErrorInfo>),
    WorkspaceSymbol(std::result::Result<Vec<SymbolInformation>, LspErrorInfo>),
    DocumentSymbol(std::result::Result<DocumentSymbolResponse, LspErrorInfo>),
    PrepareCallHierarchy(std::result::Result<Vec<CallHierarchyItem>, LspErrorInfo>),
    IncomingCalls(std::result::Result<Vec<CallHierarchyIncomingCall>, LspErrorInfo>),
    OutgoingCalls(std::result::Result<Vec<CallHierarchyOutgoingCall>, LspErrorInfo>),
}

/// Error information from LSP
#[derive(Debug, Clone)]
pub struct LspErrorInfo {
    pub code: i32,
    pub message: String,
}

/// Context for the LSP handler loop
struct LspHandlerContext {
    subscribers: Arc<RwLock<Vec<mpsc::Sender<ServerNotification>>>>,
    diagnostics_cache: Arc<RwLock<HashMap<Uri, PublishDiagnosticsParams>>>,
    root_path: PathBuf,
}

/// Spawn the LSP manager actor and return a handle
pub fn spawn_lsp_manager() -> LspManagerHandle {
    let (request_tx, request_rx) = mpsc::channel(100);
    let actor = LspManagerActor {
        lsps: HashMap::new(),
        request_rx,
    };
    tokio::spawn(actor.run());
    LspManagerHandle { request_tx }
}

impl LspManagerActor {
    /// Run the actor loop
    async fn run(mut self) {
        tracing::debug!("LspManagerActor starting");

        while let Some(request) = self.request_rx.recv().await {
            match request {
                LspManagerRequest::GetOrSpawn {
                    key,
                    command,
                    args,
                    response_tx,
                } => {
                    let result = self.get_or_spawn(key, &command, &args).await;
                    let _ = response_tx.send(result);
                }
                LspManagerRequest::Shutdown { response_tx } => {
                    self.lsps.clear();
                    let _ = response_tx.send(());
                    break;
                }
            }
        }

        tracing::debug!("LspManagerActor stopping");
    }

    /// Get or spawn an LSP for the given workspace and language
    async fn get_or_spawn(
        &mut self,
        key: LspKey,
        command: &str,
        args: &[String],
    ) -> DaemonResult<Arc<LspHandle>> {
        if let Some(handle) = self.lsps.get(&key) {
            return Ok(Arc::clone(handle));
        }

        let handle = spawn_lsp(&key.workspace_root, command, args).await?;
        let handle = Arc::new(handle);
        self.lsps.insert(key, Arc::clone(&handle));

        Ok(handle)
    }
}

impl LspManagerHandle {
    /// Get or spawn an LSP for the given workspace and language
    pub async fn get_or_spawn(
        &self,
        key: LspKey,
        command: &str,
        args: &[String],
    ) -> DaemonResult<Arc<LspHandle>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(LspManagerRequest::GetOrSpawn {
                key,
                command: command.to_string(),
                args: args.to_vec(),
                response_tx,
            })
            .await
            .map_err(|_| DaemonError::LspError("LspManager closed".into()))?;
        response_rx
            .await
            .map_err(|_| DaemonError::LspError("Response channel closed".into()))?
    }

    /// Shutdown all LSP instances
    pub async fn shutdown(&self) {
        let (response_tx, response_rx) = oneshot::channel();
        let _ = self
            .request_tx
            .send(LspManagerRequest::Shutdown { response_tx })
            .await;
        let _ = response_rx.await;
    }
}

impl LspHandle {
    /// Send a request to the LSP and wait for response
    pub async fn request(&self, request: LspOp) -> DaemonResult<LspResult> {
        let (response_tx, response_rx) = oneshot::channel();
        let envelope = LspRequestEnvelope {
            request,
            response_tx,
        };

        self.request_tx
            .send(envelope)
            .await
            .map_err(|_| DaemonError::LspError("LSP handler closed".into()))?;

        response_rx
            .await
            .map_err(|_| DaemonError::LspError("Response channel closed".into()))
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

    /// Subscribe to server notifications (diagnostics, progress, etc.)
    ///
    /// Returns a receiver that will receive notifications as they arrive.
    /// The subscription is automatically cleaned up when the receiver is dropped.
    #[allow(dead_code)]
    pub async fn subscribe(&self) -> mpsc::Receiver<ServerNotification> {
        let (tx, rx) = mpsc::channel(100);
        self.subscribers.write().await.push(tx);
        rx
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
    let subscribers = Arc::new(RwLock::new(Vec::new()));
    let diagnostics_cache = Arc::new(RwLock::new(HashMap::new()));

    let ctx = LspHandlerContext {
        subscribers: Arc::clone(&subscribers),
        diagnostics_cache: Arc::clone(&diagnostics_cache),
        root_path: root_path.to_path_buf(),
    };

    let task = tokio::spawn(async move {
        run_lsp_handler(process, stdin, stdout, request_rx, notification_rx, ctx).await;
    });

    Ok(LspHandle {
        request_tx,
        notification_tx,
        diagnostics_cache,
        subscribers,
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
    ctx: LspHandlerContext,
) {
    let mut reader = BufReader::new(stdout);
    let next_id = AtomicI64::new(1);
    let mut pending: HashMap<i64, (LspRequestKind, oneshot::Sender<LspResult>)> = HashMap::new();

    if let Err(e) = initialize_lsp(&mut stdin, &next_id, &ctx.root_path).await {
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
                        handle_lsp_message(msg, &mut pending, &ctx.subscribers, &ctx.diagnostics_cache).await;
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
                let id = next_id.fetch_add(1, Ordering::SeqCst);
                let kind = request_kind(&envelope.request);
                pending.insert(id, (kind, envelope.response_tx));

                if let Err(e) = send_lsp_request(&mut stdin, id, &envelope.request).await {
                    tracing::error!("Failed to send LSP request: {}", e);
                    if let Some((kind, tx)) = pending.remove(&id) {
                        let _ = tx.send(error_result(kind, LspErrorInfo {
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

    for (_, (kind, tx)) in pending {
        let _ = tx.send(error_result(
            kind,
            LspErrorInfo {
                code: -1,
                message: "LSP server closed".into(),
            },
        ));
    }
}

#[derive(Debug, Clone, Copy)]
enum LspRequestKind {
    GotoDefinition,
    GotoImplementation,
    FindReferences,
    Hover,
    WorkspaceSymbol,
    DocumentSymbol,
    PrepareCallHierarchy,
    IncomingCalls,
    OutgoingCalls,
}

fn request_kind(req: &LspOp) -> LspRequestKind {
    match req {
        LspOp::GotoDefinition(_) => LspRequestKind::GotoDefinition,
        LspOp::GotoImplementation(_) => LspRequestKind::GotoImplementation,
        LspOp::FindReferences(_) => LspRequestKind::FindReferences,
        LspOp::Hover(_) => LspRequestKind::Hover,
        LspOp::WorkspaceSymbol(_) => LspRequestKind::WorkspaceSymbol,
        LspOp::DocumentSymbol(_) => LspRequestKind::DocumentSymbol,
        LspOp::PrepareCallHierarchy(_) => LspRequestKind::PrepareCallHierarchy,
        LspOp::IncomingCalls(_) => LspRequestKind::IncomingCalls,
        LspOp::OutgoingCalls(_) => LspRequestKind::OutgoingCalls,
    }
}

fn error_result(kind: LspRequestKind, err: LspErrorInfo) -> LspResult {
    match kind {
        LspRequestKind::GotoDefinition => LspResult::GotoDefinition(Err(err)),
        LspRequestKind::GotoImplementation => LspResult::GotoImplementation(Err(err)),
        LspRequestKind::FindReferences => LspResult::FindReferences(Err(err)),
        LspRequestKind::Hover => LspResult::Hover(Err(err)),
        LspRequestKind::WorkspaceSymbol => LspResult::WorkspaceSymbol(Err(err)),
        LspRequestKind::DocumentSymbol => LspResult::DocumentSymbol(Err(err)),
        LspRequestKind::PrepareCallHierarchy => LspResult::PrepareCallHierarchy(Err(err)),
        LspRequestKind::IncomingCalls => LspResult::IncomingCalls(Err(err)),
        LspRequestKind::OutgoingCalls => LspResult::OutgoingCalls(Err(err)),
    }
}

/// Handle an incoming LSP message
async fn handle_lsp_message(
    msg: Value,
    pending: &mut HashMap<i64, (LspRequestKind, oneshot::Sender<LspResult>)>,
    subscribers: &RwLock<Vec<mpsc::Sender<ServerNotification>>>,
    diagnostics_cache: &RwLock<HashMap<Uri, PublishDiagnosticsParams>>,
) {
    if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
        if let Some((kind, tx)) = pending.remove(&id) {
            let result = if let Some(error) = msg.get("error") {
                let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
                let message = error
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                error_result(kind, LspErrorInfo { code, message })
            } else {
                let result = msg.get("result").cloned().unwrap_or(Value::Null);
                parse_response(kind, result)
            };
            let _ = tx.send(result);
        }
        return;
    }

    if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        let notification = match method {
            "textDocument/publishDiagnostics" => {
                if let Ok(diag_params) = serde_json::from_value::<PublishDiagnosticsParams>(params)
                {
                    let mut cache = diagnostics_cache.write().await;
                    cache.insert(diag_params.uri.clone(), diag_params.clone());

                    Some(ServerNotification::Diagnostics(diag_params))
                } else {
                    None
                }
            }
            "$/progress" => serde_json::from_value::<ProgressParams>(params)
                .ok()
                .map(ServerNotification::Progress),
            _ => None,
        };

        if let Some(notif) = notification {
            let subs = subscribers.read().await;
            for sub in subs.iter() {
                let _ = sub.try_send(notif.clone());
            }
        }
    }
}

fn parse_response(kind: LspRequestKind, value: Value) -> LspResult {
    match kind {
        LspRequestKind::GotoDefinition => LspResult::GotoDefinition(parse_or_default(
            value,
            GotoDefinitionResponse::Array(vec![]),
        )),
        LspRequestKind::GotoImplementation => LspResult::GotoImplementation(parse_or_default(
            value,
            GotoDefinitionResponse::Array(vec![]),
        )),
        LspRequestKind::FindReferences => {
            LspResult::FindReferences(parse_or_default(value, vec![]))
        }
        LspRequestKind::Hover => {
            if value.is_null() {
                LspResult::Hover(Ok(None))
            } else {
                match serde_json::from_value::<Hover>(value) {
                    Ok(h) => LspResult::Hover(Ok(Some(h))),
                    Err(e) => LspResult::Hover(Err(LspErrorInfo {
                        code: -1,
                        message: format!("Parse error: {}", e),
                    })),
                }
            }
        }
        LspRequestKind::WorkspaceSymbol => {
            LspResult::WorkspaceSymbol(parse_or_default(value, vec![]))
        }
        LspRequestKind::DocumentSymbol => LspResult::DocumentSymbol(parse_or_default(
            value,
            DocumentSymbolResponse::Flat(vec![]),
        )),
        LspRequestKind::PrepareCallHierarchy => {
            LspResult::PrepareCallHierarchy(parse_or_default(value, vec![]))
        }
        LspRequestKind::IncomingCalls => LspResult::IncomingCalls(parse_or_default(value, vec![])),
        LspRequestKind::OutgoingCalls => LspResult::OutgoingCalls(parse_or_default(value, vec![])),
    }
}

fn parse_or_default<T: DeserializeOwned>(
    value: Value,
    default: T,
) -> std::result::Result<T, LspErrorInfo> {
    if value.is_null() {
        Ok(default)
    } else {
        serde_json::from_value(value).map_err(|e| LspErrorInfo {
            code: -1,
            message: format!("Parse error: {}", e),
        })
    }
}

/// Initialize the LSP server
async fn initialize_lsp(
    stdin: &mut ChildStdin,
    next_id: &AtomicI64,
    root_path: &Path,
) -> std::io::Result<()> {
    let root_uri = path_to_uri(root_path);

    let capabilities = ClientCapabilities {
        general: Some(GeneralClientCapabilities::default()),
        window: Some(WindowClientCapabilities {
            work_done_progress: Some(true),
            ..Default::default()
        }),
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

    let id = next_id.fetch_add(1, Ordering::SeqCst);
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
        LspNotification::Opened(p) => {
            ("textDocument/didOpen", serde_json::to_value(p).unwrap())
        }
        LspNotification::Changed(p) => {
            ("textDocument/didChange", serde_json::to_value(p).unwrap())
        }
        LspNotification::Saved(p) => {
            ("textDocument/didSave", serde_json::to_value(p).unwrap())
        }
        LspNotification::Closed(p) => {
            ("textDocument/didClose", serde_json::to_value(p).unwrap())
        }
    };
    send_notification(stdin, method, params).await
}

/// Send a typed LSP request
async fn send_lsp_request(stdin: &mut ChildStdin, id: i64, request: &LspOp) -> std::io::Result<()> {
    let (method, params) = match request {
        LspOp::GotoDefinition(p) => ("textDocument/definition", serde_json::to_value(p).unwrap()),
        LspOp::GotoImplementation(p) => (
            "textDocument/implementation",
            serde_json::to_value(p).unwrap(),
        ),
        LspOp::FindReferences(p) => ("textDocument/references", serde_json::to_value(p).unwrap()),
        LspOp::Hover(p) => ("textDocument/hover", serde_json::to_value(p).unwrap()),
        LspOp::WorkspaceSymbol(p) => ("workspace/symbol", serde_json::to_value(p).unwrap()),
        LspOp::DocumentSymbol(p) => (
            "textDocument/documentSymbol",
            serde_json::to_value(p).unwrap(),
        ),
        LspOp::PrepareCallHierarchy(p) => (
            "textDocument/prepareCallHierarchy",
            serde_json::to_value(p).unwrap(),
        ),
        LspOp::IncomingCalls(p) => (
            "callHierarchy/incomingCalls",
            serde_json::to_value(p).unwrap(),
        ),
        LspOp::OutgoingCalls(p) => (
            "callHierarchy/outgoingCalls",
            serde_json::to_value(p).unwrap(),
        ),
    };

    send_request(stdin, id, method, params).await
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
mod tests {
    use super::*;

    /// Test that verifies subscribers added via LspHandle::add_subscriber are
    /// visible to the notification broadcast system.
    ///
    /// This test catches a bug where LspHandle had its own separate `subscribers`
    /// field instead of sharing the Arc with the background task.
    #[tokio::test]
    async fn test_subscribers_are_shared_with_handler() {
        let shared_subscribers: Arc<RwLock<Vec<mpsc::Sender<ServerNotification>>>> =
            Arc::new(RwLock::new(Vec::new()));

        let handler_subscribers = Arc::clone(&shared_subscribers);

        let (tx, mut rx) = mpsc::channel::<ServerNotification>(10);

        {
            let mut subs = shared_subscribers.write().await;
            subs.push(tx);
        }

        let handler_count = handler_subscribers.read().await.len();
        assert_eq!(
            handler_count, 1,
            "Handler should see subscriber added via shared Arc"
        );

        let test_notification = ServerNotification::Progress(ProgressParams {
            token: lsp_types::ProgressToken::Number(1),
            value: lsp_types::ProgressParamsValue::WorkDone(lsp_types::WorkDoneProgress::Begin(
                lsp_types::WorkDoneProgressBegin {
                    title: "test".to_string(),
                    cancellable: None,
                    message: None,
                    percentage: None,
                },
            )),
        });

        {
            let subs = handler_subscribers.read().await;
            for sub in subs.iter() {
                let _ = sub.try_send(test_notification.clone());
            }
        }

        let received = rx.try_recv();
        assert!(
            received.is_ok(),
            "Subscriber should receive notification broadcast by handler"
        );
    }

}
