use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, Location, PartialResultParams, Position,
    PublishDiagnosticsParams, ReferenceContext, ReferenceParams, SymbolInformation,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
    WorkspaceSymbolParams,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::sync::{Mutex, oneshot};

use crate::language_id::LanguageId;
use crate::protocol::{
    DaemonRequest, DaemonResponse, InitializeRequest, LspNotification, read_frame, write_frame,
};
use crate::socket_path::ensure_socket_dir;

/// Errors that can occur in the client (connecting to daemon)
#[derive(Debug, Error)]
pub enum ClientError {
    /// Failed to connect to daemon socket
    #[error("Failed to connect to daemon: {0}")]
    ConnectionFailed(#[source] io::Error),

    /// IO error during communication
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Daemon returned an error
    #[error("Daemon error: {0}")]
    DaemonError(String),

    /// LSP server returned an error
    #[error("LSP error (code={code}): {message}")]
    LspError { code: i32, message: String },

    /// Response channel was closed before receiving response
    #[error("Response channel closed")]
    ResponseChannelClosed,

    /// Failed to spawn daemon process
    #[error("Failed to spawn daemon: {0}")]
    SpawnFailed(#[source] io::Error),

    /// Timeout waiting for daemon to become available
    #[error("Timeout waiting for daemon to start")]
    SpawnTimeout,

    /// Daemon binary not found
    #[error("Daemon binary not found: {0}")]
    DaemonBinaryNotFound(String),

    /// Protocol error (unexpected message type)
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Initialization failed
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
}

/// Result type for client operations
pub type ClientResult<T> = std::result::Result<T, ClientError>;

/// Pending result: either a `Value` result or a structured error
type PendingResult = Result<Value, ClientError>;

/// Client for communicating with the LSP daemon
pub struct LspClient {
    /// Writer half of the socket
    writer: Mutex<WriteHalf<UnixStream>>,
    /// Pending responses keyed by `client_id`
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<PendingResult>>>>,
    /// Counter for generating request IDs
    next_id: AtomicI64,
    /// Background reader task handle
    reader_task: tokio::task::JoinHandle<()>,
}

/// Ensure the daemon is running for the given socket path.
///
/// Spawns the daemon if necessary. Does NOT connect - use `LspClient::connect`
/// after this if you need a connection.
pub async fn ensure_daemon_running(socket_path: &Path) -> ClientResult<()> {
    match UnixStream::connect(socket_path).await {
        Ok(_) => Ok(()),
        Err(e)
            if e.kind() == std::io::ErrorKind::ConnectionRefused
                || e.kind() == std::io::ErrorKind::NotFound =>
        {
            spawn_daemon(socket_path).await
        }
        Err(e) => Err(ClientError::ConnectionFailed(e)),
    }
}

impl LspClient {
    /// Connect to an already-running daemon
    pub async fn connect(
        socket_path: &Path,
        workspace_root: &Path,
        language: LanguageId,
    ) -> ClientResult<Self> {
        let stream = UnixStream::connect(socket_path)
            .await
            .map_err(ClientError::ConnectionFailed)?;
        Self::from_stream(stream, workspace_root, language).await
    }

    /// Connect to the daemon, spawning it if necessary
    pub async fn connect_or_spawn(
        workspace_root: &Path,
        language: LanguageId,
    ) -> ClientResult<Self> {
        let sock_path = ensure_socket_dir(workspace_root, language).map_err(ClientError::Io)?;

        match UnixStream::connect(&sock_path).await {
            Ok(stream) => {
                return Self::from_stream(stream, workspace_root, language).await;
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::ConnectionRefused
                    || e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(ClientError::ConnectionFailed(e));
            }
        }

        spawn_daemon(&sock_path).await?;

        let stream = UnixStream::connect(&sock_path)
            .await
            .map_err(ClientError::ConnectionFailed)?;

        Self::from_stream(stream, workspace_root, language).await
    }

    /// Go to the definition of a symbol at a position
    pub async fn goto_definition(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
    ) -> ClientResult<GotoDefinitionResponse> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("textDocument/definition", &params, || {
            GotoDefinitionResponse::Array(vec![])
        })
        .await
    }

    /// Go to the implementation of an interface/trait method
    pub async fn goto_implementation(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
    ) -> ClientResult<GotoDefinitionResponse> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("textDocument/implementation", &params, || {
            GotoDefinitionResponse::Array(vec![])
        })
        .await
    }

    /// Find all references to a symbol at a position
    pub async fn find_references(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> ClientResult<Vec<Location>> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration,
            },
        };
        self.call("textDocument/references", &params, Vec::new)
            .await
    }

    /// Get hover information for a symbol at a position
    pub async fn hover(&self, uri: Uri, line: u32, character: u32) -> ClientResult<Option<Hover>> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.call("textDocument/hover", &params, || None).await
    }

    /// Search for symbols across the workspace
    pub async fn workspace_symbol(&self, query: String) -> ClientResult<Vec<SymbolInformation>> {
        let params = WorkspaceSymbolParams {
            query,
            partial_result_params: PartialResultParams::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.call("workspace/symbol", &params, Vec::new).await
    }

    /// Get all symbols in a document
    pub async fn document_symbol(&self, uri: Uri) -> ClientResult<DocumentSymbolResponse> {
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("textDocument/documentSymbol", &params, || {
            DocumentSymbolResponse::Flat(vec![])
        })
        .await
    }

    /// Prepare call hierarchy at a position
    pub async fn prepare_call_hierarchy(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
    ) -> ClientResult<Vec<CallHierarchyItem>> {
        let params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.call("textDocument/prepareCallHierarchy", &params, Vec::new)
            .await
    }

    /// Get incoming calls for a call hierarchy item
    pub async fn incoming_calls(
        &self,
        item: CallHierarchyItem,
    ) -> ClientResult<Vec<CallHierarchyIncomingCall>> {
        let params = CallHierarchyIncomingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("callHierarchy/incomingCalls", &params, Vec::new)
            .await
    }

    /// Get outgoing calls for a call hierarchy item
    pub async fn outgoing_calls(
        &self,
        item: CallHierarchyItem,
    ) -> ClientResult<Vec<CallHierarchyOutgoingCall>> {
        let params = CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.call("callHierarchy/outgoingCalls", &params, Vec::new)
            .await
    }

    /// Get cached diagnostics from the daemon
    ///
    /// If `uri` is Some, returns diagnostics for that file only.
    /// If `uri` is None, returns all cached diagnostics for the workspace.
    pub async fn get_diagnostics(
        &self,
        uri: Option<Uri>,
    ) -> ClientResult<Vec<PublishDiagnosticsParams>> {
        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::GetDiagnostics { client_id, uri };

        self.send_and_await(request, client_id).await.and_then(|v| {
            serde_json::from_value(v).map_err(|e| ClientError::ProtocolError(e.to_string()))
        })
    }

    /// Send a notification (fire-and-forget)
    pub async fn send_notification(
        &self,
        method: &str,
        params: &impl Serialize,
    ) -> ClientResult<()> {
        let notification = LspNotification {
            method: method.to_string(),
            params: serde_json::to_value(params)
                .map_err(|e| ClientError::ProtocolError(e.to_string()))?,
        };
        let request = DaemonRequest::LspNotification(notification);
        let mut writer = self.writer.lock().await;
        write_frame(&mut *writer, &request)
            .await
            .map_err(ClientError::Io)
    }

    /// Send notification that a document was opened
    pub async fn notify_opened(&self, params: DidOpenTextDocumentParams) -> ClientResult<()> {
        self.send_notification("textDocument/didOpen", &params)
            .await
    }

    /// Send notification that a document was changed
    pub async fn notify_changed(&self, params: DidChangeTextDocumentParams) -> ClientResult<()> {
        self.send_notification("textDocument/didChange", &params)
            .await
    }

    /// Send notification that a document was saved
    pub async fn notify_saved(&self, params: DidSaveTextDocumentParams) -> ClientResult<()> {
        self.send_notification("textDocument/didSave", &params)
            .await
    }

    /// Send notification that a document was closed
    pub async fn notify_closed(&self, params: DidCloseTextDocumentParams) -> ClientResult<()> {
        self.send_notification("textDocument/didClose", &params)
            .await
    }

    /// Gracefully disconnect from the daemon
    pub async fn disconnect(self) -> ClientResult<()> {
        let request = DaemonRequest::Disconnect;
        let mut writer = self.writer.lock().await;
        write_frame(&mut *writer, &request)
            .await
            .map_err(ClientError::Io)
    }
}

impl LspClient {
    /// Create client from an existing stream
    async fn from_stream(
        stream: UnixStream,
        workspace_root: &Path,
        language: LanguageId,
    ) -> ClientResult<Self> {
        let (mut reader, mut writer) = tokio::io::split(stream);

        let init_request = DaemonRequest::Initialize(InitializeRequest {
            workspace_root: workspace_root.to_path_buf(),
            language,
        });

        write_frame(&mut writer, &init_request)
            .await
            .map_err(ClientError::Io)?;

        let response: Option<DaemonResponse> =
            read_frame(&mut reader).await.map_err(ClientError::Io)?;

        match response {
            Some(DaemonResponse::Initialized) => {}
            Some(DaemonResponse::Error(e)) => {
                return Err(ClientError::InitializationFailed(e.message));
            }
            Some(_) => {
                return Err(ClientError::ProtocolError(
                    "Unexpected response to Initialize".into(),
                ));
            }
            None => {
                return Err(ClientError::ProtocolError(
                    "Connection closed during initialization".into(),
                ));
            }
        }

        let pending: Arc<Mutex<HashMap<i64, oneshot::Sender<PendingResult>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let pending_clone = Arc::clone(&pending);
        let reader_task = tokio::spawn(async move {
            run_reader(reader, pending_clone).await;
        });

        Ok(Self {
            writer: Mutex::new(writer),
            pending,
            next_id: AtomicI64::new(1),
            reader_task,
        })
    }

    /// Send an LSP call and deserialize the response.
    ///
    /// Serializes `params`, sends `DaemonRequest::LspCall`, awaits the `Value`
    /// result, and deserializes into `R` (using `default()` for null responses).
    pub async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: &P,
        default: impl FnOnce() -> R,
    ) -> ClientResult<R> {
        let params_value =
            serde_json::to_value(params).map_err(|e| ClientError::ProtocolError(e.to_string()))?;

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::LspCall {
            client_id,
            method: method.to_string(),
            params: params_value,
        };

        let value = self.send_and_await(request, client_id).await?;

        if value.is_null() {
            Ok(default())
        } else {
            serde_json::from_value(value)
                .map_err(|e| ClientError::ProtocolError(format!("Parse error: {e}")))
        }
    }

    /// Send a request to the daemon and await the `Value` result.
    async fn send_and_await(&self, request: DaemonRequest, client_id: i64) -> ClientResult<Value> {
        let (response_tx, response_rx) = oneshot::channel();

        {
            let mut pending = self.pending.lock().await;
            pending.insert(client_id, response_tx);
        }

        {
            let mut writer = self.writer.lock().await;
            write_frame(&mut *writer, &request).await.map_err(|e| {
                let pending = self.pending.clone();
                tokio::spawn(async move {
                    pending.lock().await.remove(&client_id);
                });
                ClientError::Io(e)
            })?;
        }

        response_rx
            .await
            .map_err(|_| ClientError::ProtocolError("Response channel closed".into()))?
    }
}

/// Run the reader task
async fn run_reader(
    mut reader: ReadHalf<UnixStream>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<PendingResult>>>>,
) {
    loop {
        let response: Option<DaemonResponse> = match read_frame(&mut reader).await {
            Ok(Some(resp)) => Some(resp),
            Ok(None) => {
                tracing::debug!("Daemon connection closed");
                break;
            }
            Err(e) => {
                tracing::debug!("Error reading from daemon: {}", e);
                break;
            }
        };

        match response {
            Some(DaemonResponse::LspResult { client_id, result }) => {
                let mut pending = pending.lock().await;
                if let Some(tx) = pending.remove(&client_id) {
                    let value_result = result.map_err(|e| ClientError::LspError {
                        code: e.code,
                        message: e.message,
                    });
                    let _ = tx.send(value_result);
                }
            }
            Some(DaemonResponse::Error(err)) => {
                if let Some(client_id) = err.client_id {
                    let mut pending = pending.lock().await;
                    if let Some(tx) = pending.remove(&client_id) {
                        let _ = tx.send(Err(ClientError::DaemonError(err.message)));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Spawn the daemon process
async fn spawn_daemon(socket_path: &Path) -> ClientResult<()> {
    let daemon_path = find_daemon_binary()?;

    let mut child = Command::new(&daemon_path)
        .arg("--socket")
        .arg(socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(ClientError::SpawnFailed)?;

    // Wait for daemon to become available, checking for early exit
    for _ in 0..50 {
        // Check if process exited early (indicates startup failure)
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                return Err(ClientError::SpawnFailed(std::io::Error::other(format!(
                    "Daemon exited with status: {status}"
                ))));
            }
            Ok(Some(_) | None) => {
                // Process exited successfully - daemon daemonized, or still running; continue waiting for socket
            }
            Err(e) => {
                return Err(ClientError::SpawnFailed(e));
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
        if UnixStream::connect(socket_path).await.is_ok() {
            return Ok(());
        }
    }

    Err(ClientError::SpawnTimeout)
}

/// Find the daemon binary
fn find_daemon_binary() -> ClientResult<PathBuf> {
    let candidates = [
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("aether-lspd"))),
        std::env::current_exe().ok().and_then(|p| {
            p.parent()
                .and_then(|d| d.parent())
                .map(|d| d.join("aether-lspd"))
        }),
        which_aether_lspd(),
        Some(PathBuf::from("target/debug/aether-lspd")),
        Some(PathBuf::from("target/release/aether-lspd")),
        Some(PathBuf::from("../../target/debug/aether-lspd")),
        Some(PathBuf::from("../../target/release/aether-lspd")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(ClientError::DaemonBinaryNotFound(
        "aether-lspd not found".into(),
    ))
}

/// Try to find aether-lspd in PATH
fn which_aether_lspd() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join("aether-lspd"))
            .find(|p| p.exists())
    })
}

impl Drop for LspClient {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}
