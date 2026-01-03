use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse,
    Hover, HoverParams, Location, Position, ReferenceContext, ReferenceParams, SymbolInformation,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkspaceSymbolParams,
};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::sync::{Mutex, oneshot};

use aether_lspd::{
    DaemonRequest, DaemonResponse, InitializeRequest, LanguageId, LspNotification, LspRequest,
    LspResponse, read_frame, write_frame,
};

use super::super::config::LspConfig;
use super::error::{DaemonClientError, Result};
use super::socket_path::ensure_socket_dir;

/// Client for communicating with the LSP daemon
pub struct LspDaemonClient {
    /// Writer half of the socket
    writer: Mutex<WriteHalf<UnixStream>>,
    /// Pending responses keyed by client_id
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<LspResponse>>>>,
    /// Counter for generating request IDs
    next_id: AtomicI64,
    /// Background reader task handle
    _reader_task: tokio::task::JoinHandle<()>,
}

impl LspDaemonClient {
    /// Connect to the daemon, spawning it if necessary
    pub async fn connect_or_spawn(
        workspace_root: &Path,
        language: LanguageId,
        config: &LspConfig,
    ) -> Result<Self> {
        let sock_path =
            ensure_socket_dir(workspace_root, language).map_err(DaemonClientError::Io)?;

        match UnixStream::connect(&sock_path).await {
            Ok(stream) => {
                return Self::from_stream(stream, workspace_root, language, config).await;
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::ConnectionRefused
                    || e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(DaemonClientError::ConnectionFailed(e));
            }
        }

        spawn_daemon(&sock_path).await?;

        let stream = UnixStream::connect(&sock_path)
            .await
            .map_err(DaemonClientError::ConnectionFailed)?;

        Self::from_stream(stream, workspace_root, language, config).await
    }

    /// Create client from an existing stream
    async fn from_stream(
        stream: UnixStream,
        workspace_root: &Path,
        language: LanguageId,
        config: &LspConfig,
    ) -> Result<Self> {
        let (mut reader, mut writer) = tokio::io::split(stream);

        let init_request = DaemonRequest::Initialize(InitializeRequest {
            workspace_root: workspace_root.to_path_buf(),
            language,
            lsp_command: config.command.clone(),
            lsp_args: config.args.clone(),
        });

        write_frame(&mut writer, &init_request)
            .await
            .map_err(DaemonClientError::Io)?;

        let response: Option<DaemonResponse> = read_frame(&mut reader)
            .await
            .map_err(DaemonClientError::Io)?;

        match response {
            Some(DaemonResponse::Initialized) => {}
            Some(DaemonResponse::Error(e)) => {
                return Err(DaemonClientError::InitializationFailed(e.message));
            }
            Some(_) => {
                return Err(DaemonClientError::ProtocolError(
                    "Unexpected response to Initialize".into(),
                ));
            }
            None => {
                return Err(DaemonClientError::ProtocolError(
                    "Connection closed during initialization".into(),
                ));
            }
        }

        let pending: Arc<Mutex<HashMap<i64, oneshot::Sender<LspResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let pending_clone = Arc::clone(&pending);
        let reader_task = tokio::spawn(async move {
            run_reader(reader, pending_clone).await;
        });

        Ok(Self {
            writer: Mutex::new(writer),
            pending,
            next_id: AtomicI64::new(1),
            _reader_task: reader_task,
        })
    }

    /// Go to the definition of a symbol at a position
    pub async fn goto_definition(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
    ) -> Result<GotoDefinitionResponse> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::LspRequest(LspRequest::GotoDefinition { client_id, params });

        self.send_lsp_request(request, client_id)
            .await
            .and_then(|resp| match resp {
                LspResponse::GotoDefinition { result, .. } => {
                    result.map_err(|e| DaemonClientError::LspError {
                        code: e.code,
                        message: e.message,
                    })
                }
                _ => Err(DaemonClientError::ProtocolError(
                    "Unexpected response type".into(),
                )),
            })
    }

    /// Go to the implementation of an interface/trait method
    pub async fn goto_implementation(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
    ) -> Result<GotoDefinitionResponse> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request =
            DaemonRequest::LspRequest(LspRequest::GotoImplementation { client_id, params });

        self.send_lsp_request(request, client_id)
            .await
            .and_then(|resp| match resp {
                LspResponse::GotoImplementation { result, .. } => {
                    result.map_err(|e| DaemonClientError::LspError {
                        code: e.code,
                        message: e.message,
                    })
                }
                _ => Err(DaemonClientError::ProtocolError(
                    "Unexpected response type".into(),
                )),
            })
    }

    /// Find all references to a symbol at a position
    pub async fn find_references(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Vec<Location>> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: ReferenceContext {
                include_declaration,
            },
        };

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::LspRequest(LspRequest::FindReferences { client_id, params });

        self.send_lsp_request(request, client_id)
            .await
            .and_then(|resp| match resp {
                LspResponse::FindReferences { result, .. } => {
                    result.map_err(|e| DaemonClientError::LspError {
                        code: e.code,
                        message: e.message,
                    })
                }
                _ => Err(DaemonClientError::ProtocolError(
                    "Unexpected response type".into(),
                )),
            })
    }

    /// Get hover information for a symbol at a position
    pub async fn hover(&self, uri: Uri, line: u32, character: u32) -> Result<Option<Hover>> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
        };

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::LspRequest(LspRequest::Hover { client_id, params });

        self.send_lsp_request(request, client_id)
            .await
            .and_then(|resp| match resp {
                LspResponse::Hover { result, .. } => {
                    result.map_err(|e| DaemonClientError::LspError {
                        code: e.code,
                        message: e.message,
                    })
                }
                _ => Err(DaemonClientError::ProtocolError(
                    "Unexpected response type".into(),
                )),
            })
    }

    /// Search for symbols across the workspace
    pub async fn workspace_symbol(&self, query: String) -> Result<Vec<SymbolInformation>> {
        let params = WorkspaceSymbolParams {
            query,
            partial_result_params: Default::default(),
            work_done_progress_params: Default::default(),
        };

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::LspRequest(LspRequest::WorkspaceSymbol { client_id, params });

        self.send_lsp_request(request, client_id)
            .await
            .and_then(|resp| match resp {
                LspResponse::WorkspaceSymbol { result, .. } => {
                    result.map_err(|e| DaemonClientError::LspError {
                        code: e.code,
                        message: e.message,
                    })
                }
                _ => Err(DaemonClientError::ProtocolError(
                    "Unexpected response type".into(),
                )),
            })
    }

    /// Get all symbols in a document
    pub async fn document_symbol(&self, uri: Uri) -> Result<DocumentSymbolResponse> {
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::LspRequest(LspRequest::DocumentSymbol { client_id, params });

        self.send_lsp_request(request, client_id)
            .await
            .and_then(|resp| match resp {
                LspResponse::DocumentSymbol { result, .. } => {
                    result.map_err(|e| DaemonClientError::LspError {
                        code: e.code,
                        message: e.message,
                    })
                }
                _ => Err(DaemonClientError::ProtocolError(
                    "Unexpected response type".into(),
                )),
            })
    }

    /// Prepare call hierarchy at a position
    pub async fn prepare_call_hierarchy(
        &self,
        uri: Uri,
        line: u32,
        character: u32,
    ) -> Result<Vec<CallHierarchyItem>> {
        let params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
        };

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request =
            DaemonRequest::LspRequest(LspRequest::PrepareCallHierarchy { client_id, params });

        self.send_lsp_request(request, client_id)
            .await
            .and_then(|resp| match resp {
                LspResponse::PrepareCallHierarchy { result, .. } => {
                    result.map_err(|e| DaemonClientError::LspError {
                        code: e.code,
                        message: e.message,
                    })
                }
                _ => Err(DaemonClientError::ProtocolError(
                    "Unexpected response type".into(),
                )),
            })
    }

    /// Get incoming calls for a call hierarchy item
    pub async fn incoming_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyIncomingCall>> {
        let params = CallHierarchyIncomingCallsParams {
            item,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::LspRequest(LspRequest::IncomingCalls { client_id, params });

        self.send_lsp_request(request, client_id)
            .await
            .and_then(|resp| match resp {
                LspResponse::IncomingCalls { result, .. } => {
                    result.map_err(|e| DaemonClientError::LspError {
                        code: e.code,
                        message: e.message,
                    })
                }
                _ => Err(DaemonClientError::ProtocolError(
                    "Unexpected response type".into(),
                )),
            })
    }

    /// Get outgoing calls for a call hierarchy item
    pub async fn outgoing_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyOutgoingCall>> {
        let params = CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let client_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest::LspRequest(LspRequest::OutgoingCalls { client_id, params });

        self.send_lsp_request(request, client_id)
            .await
            .and_then(|resp| match resp {
                LspResponse::OutgoingCalls { result, .. } => {
                    result.map_err(|e| DaemonClientError::LspError {
                        code: e.code,
                        message: e.message,
                    })
                }
                _ => Err(DaemonClientError::ProtocolError(
                    "Unexpected response type".into(),
                )),
            })
    }

    /// Send a notification (fire-and-forget)
    pub async fn send_notification(&self, notification: LspNotification) -> Result<()> {
        let request = DaemonRequest::LspNotification(notification);
        let mut writer = self.writer.lock().await;
        write_frame(&mut *writer, &request)
            .await
            .map_err(DaemonClientError::Io)
    }

    /// Send notification that a document was opened
    pub async fn notify_opened(&self, params: DidOpenTextDocumentParams) -> Result<()> {
        self.send_notification(LspNotification::TextDocumentOpened(params))
            .await
    }

    /// Send notification that a document was changed
    pub async fn notify_changed(&self, params: DidChangeTextDocumentParams) -> Result<()> {
        self.send_notification(LspNotification::TextDocumentChanged(params))
            .await
    }

    /// Send notification that a document was saved
    pub async fn notify_saved(&self, params: DidSaveTextDocumentParams) -> Result<()> {
        self.send_notification(LspNotification::TextDocumentSaved(params))
            .await
    }

    /// Internal method to send an LSP request and wait for response
    async fn send_lsp_request(
        &self,
        request: DaemonRequest,
        client_id: i64,
    ) -> Result<LspResponse> {
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
                DaemonClientError::Io(e)
            })?;
        }

        response_rx
            .await
            .map_err(|_| DaemonClientError::ProtocolError("Response channel closed".into()))
    }
}

/// Run the reader task
async fn run_reader(
    mut reader: ReadHalf<UnixStream>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<LspResponse>>>>,
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

        if let Some(DaemonResponse::LspResponse(lsp_resp)) = response {
            let client_id = lsp_resp.client_id();
            let mut pending = pending.lock().await;
            if let Some(tx) = pending.remove(&client_id) {
                let _ = tx.send(lsp_resp);
            }
        }
    }
}

/// Spawn the daemon process
async fn spawn_daemon(socket_path: &Path) -> Result<()> {
    let daemon_path = find_daemon_binary()?;

    Command::new(&daemon_path)
        .arg("--socket")
        .arg(socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(DaemonClientError::SpawnFailed)?;

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if UnixStream::connect(socket_path).await.is_ok() {
            return Ok(());
        }
    }

    Err(DaemonClientError::SpawnTimeout)
}

/// Find the daemon binary
fn find_daemon_binary() -> Result<PathBuf> {
    let candidates = [
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("aether-lspd"))),
        which_aether_lspd(),
        Some(PathBuf::from("target/debug/aether-lspd")),
        Some(PathBuf::from("target/release/aether-lspd")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(DaemonClientError::DaemonBinaryNotFound(
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
