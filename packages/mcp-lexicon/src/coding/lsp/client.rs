use super::error::{LspError, Result};
use super::transport::{
    ClientNotification, ClientRequest, ParsedMessage, ParsedNotification, read_message,
    send_notification, send_request,
};
use lsp_types::{
    ClientCapabilities, DynamicRegistrationClientCapabilities, GeneralClientCapabilities,
    GotoCapability, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverClientCapabilities,
    HoverParams, InitializeParams, Location, MarkupKind, Position, ProgressParams,
    PublishDiagnosticsClientCapabilities, PublishDiagnosticsParams, ReferenceContext,
    ReferenceParams, SymbolInformation, TextDocumentClientCapabilities, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri, WindowClientCapabilities, WorkspaceSymbolParams,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Stdio, id};
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::spawn;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// Notifications sent from server to client
#[derive(Debug, Clone)]
pub enum ServerNotification {
    /// Diagnostics published for a document (errors, warnings, etc.)
    Diagnostics(PublishDiagnosticsParams),
    /// Progress updates (indexing, loading workspace, etc.)
    Progress(ProgressParams),
}

/// Requests sent from client to server (expects a response)
enum Request {
    Initialize {
        id: i64,
        params: Box<InitializeParams>,
        response_tx: oneshot::Sender<Result<()>>,
    },
    Shutdown {
        id: i64,
        response_tx: oneshot::Sender<Result<()>>,
    },
    GotoDefinition {
        id: i64,
        params: GotoDefinitionParams,
        response_tx: oneshot::Sender<Result<GotoDefinitionResponse>>,
    },
    FindReferences {
        id: i64,
        params: ReferenceParams,
        response_tx: oneshot::Sender<Result<Vec<Location>>>,
    },
    Hover {
        id: i64,
        params: HoverParams,
        response_tx: oneshot::Sender<Result<Option<Hover>>>,
    },
    WorkspaceSymbol {
        id: i64,
        params: WorkspaceSymbolParams,
        response_tx: oneshot::Sender<Result<Vec<SymbolInformation>>>,
    },
}

/// Pending response channels, keyed by request ID
enum PendingResponse {
    Initialize(oneshot::Sender<Result<()>>),
    Shutdown(oneshot::Sender<Result<()>>),
    GotoDefinition(oneshot::Sender<Result<GotoDefinitionResponse>>),
    FindReferences(oneshot::Sender<Result<Vec<Location>>>),
    Hover(oneshot::Sender<Result<Option<Hover>>>),
    WorkspaceSymbol(oneshot::Sender<Result<Vec<SymbolInformation>>>),
}

/// An LSP client that manages a language server process
///
/// The client handles:
/// - Spawning and managing the server process
/// - JSON-RPC request/response correlation
/// - LSP lifecycle (initialize, initialized, shutdown)
///
/// Use the notification channels returned from `spawn()` for sending/receiving notifications.
/// Diagnostics caching is the caller's responsibility - listen to `NotificationReceiver` for
/// `ServerNotification::Diagnostics` events and cache them as needed.
#[derive(Debug)]
pub struct LspClient {
    /// Counter for generating unique request IDs
    request_id: AtomicI64,
    /// Channel to send requests to the handler task
    request_tx: Sender<Request>,
    /// Channel to send notifications (internal clone)
    notification_tx: Sender<ClientNotification>,
    /// Handle to the handler task
    _task_handle: JoinHandle<()>,
}

/// Sender for client-to-server notifications
pub type NotificationSender = Sender<ClientNotification>;
/// Receiver for server-to-client notifications
pub type NotificationReceiver = Receiver<ServerNotification>;

impl LspClient {
    /// Spawn and initialize a new language server process
    ///
    /// Returns a tuple of:
    /// - `NotificationSender`: Send notifications to the server (didOpen, didChange, etc.)
    /// - `NotificationReceiver`: Receive notifications from the server (diagnostics, progress)
    /// - `LspClient`: Handle for requests and accessing the diagnostics cache
    ///
    /// # Arguments
    /// * `command` - The command to run (e.g., "rust-analyzer")
    /// * `args` - Arguments to pass to the command
    /// * `root_path` - The root directory of the project
    ///
    /// # Example
    /// ```ignore
    /// let (tx, rx, client) = LspClient::spawn("rust-analyzer", &[], &root_path).await?;
    /// tx.send(ClientNotification::TextDocumentOpened(params)).await?;
    /// ```
    pub async fn spawn(
        command: &str,
        args: &[&str],
        root_path: &Path,
    ) -> Result<(NotificationSender, NotificationReceiver, Self)> {
        let mut process = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| LspError::Transport("Failed to capture stdin".into()))?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| LspError::Transport("Failed to capture stdout".into()))?;

        let (request_tx, request_rx) = channel(100);
        let (client_notif_tx, client_notif_rx) = channel(100);
        let (server_notif_tx, server_notif_rx) = channel(200);

        let task_handle = spawn(run_handler(
            process,
            stdin,
            stdout,
            request_rx,
            client_notif_rx,
            server_notif_tx,
        ));

        let mut client = Self {
            request_id: AtomicI64::new(1),
            request_tx,
            notification_tx: client_notif_tx.clone(),
            _task_handle: task_handle,
        };

        client.initialize(root_path).await?;

        Ok((client_notif_tx, server_notif_rx, client))
    }

    /// Initialize the language server (called internally by spawn)
    async fn initialize(&mut self, root_path: &Path) -> Result<()> {
        let root_uri = path_to_uri(root_path)?;
        let general_capabilities = GeneralClientCapabilities {
            ..Default::default()
        };

        let window_capabilities = WindowClientCapabilities {
            work_done_progress: Some(true),
            ..Default::default()
        };

        let pub_diagnostic_capabilities = PublishDiagnosticsClientCapabilities {
            related_information: Some(true),
            ..Default::default()
        };

        // Declare support for definition capability
        let definition_capability = GotoCapability {
            dynamic_registration: Some(false),
            link_support: Some(true),
        };

        // Declare support for references capability
        let references_capability = DynamicRegistrationClientCapabilities {
            dynamic_registration: Some(false),
        };

        // Declare support for hover capability
        let hover_capability = HoverClientCapabilities {
            dynamic_registration: Some(false),
            content_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
        };

        let text_document_capabilities = TextDocumentClientCapabilities {
            publish_diagnostics: Some(pub_diagnostic_capabilities),
            definition: Some(definition_capability),
            references: Some(references_capability),
            hover: Some(hover_capability),
            ..Default::default()
        };

        let capabilities = ClientCapabilities {
            general: Some(general_capabilities),
            window: Some(window_capabilities),
            text_document: Some(text_document_capabilities),
            ..Default::default()
        };

        let params = InitializeParams {
            process_id: Some(id()),
            #[allow(deprecated)]
            root_uri: Some(root_uri.clone()),
            capabilities,
            ..Default::default()
        };

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::Initialize {
                id,
                params: Box::new(params),
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx
            .await
            .map_err(|_| LspError::response_closed())??;

        self.notification_tx
            .try_send(ClientNotification::Initialized)
            .map_err(|_| LspError::handler_closed())?;

        Ok(())
    }

    /// Shutdown the language server gracefully
    ///
    /// This sends the `shutdown` request followed by the `exit` notification,
    /// then waits for the process to terminate.
    pub async fn shutdown(&mut self) -> Result<()> {
        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::Shutdown { id, response_tx })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx
            .await
            .map_err(|_| LspError::response_closed())??;

        self.notification_tx
            .try_send(ClientNotification::Exit)
            .map_err(|_| LspError::handler_closed())?;
        Ok(())
    }

    /// Go to the definition of a symbol at a position
    ///
    /// # Arguments
    /// * `uri` - The URI of the document
    /// * `line` - Line number (0-indexed)
    /// * `character` - Character offset (0-indexed)
    ///
    /// # Returns
    /// The definition response, which may be a single location, multiple locations,
    /// or location links depending on the server.
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

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::GotoDefinition {
                id,
                params,
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx
            .await
            .map_err(|_| LspError::response_closed())?
    }

    /// Find all references to a symbol at a position
    ///
    /// # Arguments
    /// * `uri` - The URI of the document
    /// * `line` - Line number (0-indexed)
    /// * `character` - Character offset (0-indexed)
    /// * `include_declaration` - Whether to include the declaration in the results
    ///
    /// # Returns
    /// A list of locations where the symbol is referenced.
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

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::FindReferences {
                id,
                params,
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx
            .await
            .map_err(|_| LspError::response_closed())?
    }

    /// Get hover information (type, documentation) for a symbol at a position
    ///
    /// # Arguments
    /// * `uri` - The URI of the document
    /// * `line` - Line number (0-indexed)
    /// * `character` - Character offset (0-indexed)
    ///
    /// # Returns
    /// Hover information if available, or None if no information at the position.
    pub async fn hover(&self, uri: Uri, line: u32, character: u32) -> Result<Option<Hover>> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
        };

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::Hover {
                id,
                params,
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx
            .await
            .map_err(|_| LspError::response_closed())?
    }

    /// Search for symbols across the workspace
    ///
    /// # Arguments
    /// * `query` - The search query (fuzzy matching is used by most language servers)
    ///
    /// # Returns
    /// A list of symbols matching the query, including their locations and kinds.
    pub async fn workspace_symbol(&self, query: String) -> Result<Vec<SymbolInformation>> {
        let params = WorkspaceSymbolParams {
            query,
            partial_result_params: Default::default(),
            work_done_progress_params: Default::default(),
        };

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::WorkspaceSymbol {
                id,
                params,
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx
            .await
            .map_err(|_| LspError::response_closed())?
    }

    /// Generate the next unique request ID
    fn next_request_id(&self) -> i64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }
}

/// The handler task that owns stdin, stdout, and the pending requests map
async fn run_handler(
    mut process: Child,
    mut stdin: ChildStdin,
    stdout: ChildStdout,
    mut request_rx: Receiver<Request>,
    mut notification_rx: Receiver<ClientNotification>,
    server_notif_tx: Sender<ServerNotification>,
) {
    let mut reader = BufReader::new(stdout);
    let mut pending: HashMap<i64, PendingResponse> = HashMap::new();

    loop {
        tokio::select! {
            // Handle message from LSP
            msg = read_message(&mut reader) => {
                match msg {
                    Ok(Some(ParsedMessage::Response(resp))) => handle_response(resp, &mut pending),
                    Ok(Some(ParsedMessage::Notification(notif))) => {
                        handle_server_notification(notif, &server_notif_tx);
                    }
                    Ok(None) => {}
                    Err(LspError::Transport(ref s)) if s.contains("closed") => break,
                    Err(e) => tracing::warn!("Error reading LSP message: {}", e),
                }
            }

            Some(req) = request_rx.recv() => {
                let (client_request, pending_response) = match req {
                    Request::Initialize { id, params, response_tx } => (
                        ClientRequest::Initialize(id, params),
                        PendingResponse::Initialize(response_tx),
                    ),
                    Request::Shutdown { id, response_tx } => (
                        ClientRequest::Shutdown(id),
                        PendingResponse::Shutdown(response_tx),
                    ),
                    Request::GotoDefinition { id, params, response_tx } => (
                        ClientRequest::GotoDefinition(id, params),
                        PendingResponse::GotoDefinition(response_tx),
                    ),
                    Request::FindReferences { id, params, response_tx } => (
                        ClientRequest::FindReferences(id, params),
                        PendingResponse::FindReferences(response_tx),
                    ),
                    Request::Hover { id, params, response_tx } => (
                        ClientRequest::Hover(id, params),
                        PendingResponse::Hover(response_tx),
                    ),
                    Request::WorkspaceSymbol { id, params, response_tx } => (
                        ClientRequest::WorkspaceSymbol(id, params),
                        PendingResponse::WorkspaceSymbol(response_tx),
                    ),
                };

                let id = client_request.id();
                pending.insert(id, pending_response);
                if let Err(e) = send_request(&mut stdin, &client_request).await
                    && let Some(p) = pending.remove(&id) {
                        send_error(p, e);
                    }
            }

            Some(notif) = notification_rx.recv() => {
                let _ = send_notification(&mut stdin, &notif).await;
            }

            // Handle process exit
            _ = process.wait() => {
                break;
            }
        }
    }

    // Notify any remaining pending requests that the handler is closing
    for (_, p) in pending {
        send_error(p, LspError::Transport("Handler task closed".into()));
    }

    // Explicitly kill the process to avoid orphaned LSP processes.
    let _ = process.kill().await;
}

/// Send a successful response to the appropriate typed channel
fn send_response(pending: PendingResponse, value: Value) {
    match pending {
        PendingResponse::Initialize(tx) => {
            let _ = tx.send(Ok(()));
        }
        PendingResponse::Shutdown(tx) => {
            let _ = tx.send(Ok(()));
        }
        PendingResponse::GotoDefinition(tx) => {
            // Parse the response - can be null, Location, Location[], or LocationLink[]
            let result = if value.is_null() {
                Ok(GotoDefinitionResponse::Array(vec![]))
            } else {
                serde_json::from_value::<GotoDefinitionResponse>(value).map_err(|e| {
                    LspError::InvalidMessage(format!("Failed to parse definition response: {}", e))
                })
            };
            let _ = tx.send(result);
        }
        PendingResponse::FindReferences(tx) => {
            // Parse the response - can be null or Location[]
            let result = if value.is_null() {
                Ok(vec![])
            } else {
                serde_json::from_value::<Vec<Location>>(value).map_err(|e| {
                    LspError::InvalidMessage(format!("Failed to parse references response: {}", e))
                })
            };
            let _ = tx.send(result);
        }
        PendingResponse::Hover(tx) => {
            // Parse the response - can be null or Hover
            let result = if value.is_null() {
                Ok(None)
            } else {
                serde_json::from_value::<Hover>(value)
                    .map(Some)
                    .map_err(|e| {
                        LspError::InvalidMessage(format!("Failed to parse hover response: {}", e))
                    })
            };
            let _ = tx.send(result);
        }
        PendingResponse::WorkspaceSymbol(tx) => {
            // Parse the response - can be null or SymbolInformation[]
            let result = if value.is_null() {
                Ok(vec![])
            } else {
                serde_json::from_value::<Vec<SymbolInformation>>(value).map_err(|e| {
                    LspError::InvalidMessage(format!(
                        "Failed to parse workspace symbol response: {}",
                        e
                    ))
                })
            };
            let _ = tx.send(result);
        }
    }
}

/// Send an error to the appropriate typed channel
fn send_error(pending: PendingResponse, err: LspError) {
    match pending {
        PendingResponse::Initialize(tx) => {
            let _ = tx.send(Err(err));
        }
        PendingResponse::Shutdown(tx) => {
            let _ = tx.send(Err(err));
        }
        PendingResponse::GotoDefinition(tx) => {
            let _ = tx.send(Err(err));
        }
        PendingResponse::FindReferences(tx) => {
            let _ = tx.send(Err(err));
        }
        PendingResponse::Hover(tx) => {
            let _ = tx.send(Err(err));
        }
        PendingResponse::WorkspaceSymbol(tx) => {
            let _ = tx.send(Err(err));
        }
    }
}

/// Handle a response from the server, matching it to a pending request
fn handle_response(
    resp: super::transport::ResponseMessage,
    pending: &mut HashMap<i64, PendingResponse>,
) {
    let Some(pending_response) = pending.remove(&resp.id) else {
        return;
    };

    match resp.error {
        Some(e) => send_error(
            pending_response,
            LspError::ServerError {
                code: e.code,
                message: e.message,
            },
        ),
        None => send_response(pending_response, resp.result.unwrap_or(Value::Null)),
    }
}

/// Handle a typed server notification, forwarding to the notification channel
fn handle_server_notification(
    notification: ParsedNotification,
    notification_tx: &mpsc::Sender<ServerNotification>,
) {
    match notification {
        ParsedNotification::Diagnostics(params) => {
            let _ = notification_tx.try_send(ServerNotification::Diagnostics(params));
        }
        ParsedNotification::Progress(params) => {
            let _ = notification_tx.try_send(ServerNotification::Progress(params));
        }
        ParsedNotification::Unknown(_method) => {
            // Unknown notification, ignore
        }
    }
}

/// Convert a file path to an LSP URI
///
/// This creates a `file://` URI from an absolute path.
/// On Windows, paths are converted to the correct format.
pub fn path_to_uri(path: &Path) -> Result<Uri> {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| LspError::Transport(format!("Failed to get current directory: {}", e)))?
            .join(path)
    };

    // Format as file URI
    // On Unix: file:///path/to/file
    // On Windows: file:///C:/path/to/file
    let uri_str = if cfg!(windows) {
        format!(
            "file:///{}",
            absolute_path.to_string_lossy().replace('\\', "/")
        )
    } else {
        format!("file://{}", absolute_path.display())
    };

    uri_str
        .parse()
        .map_err(|e| LspError::Transport(format!("Failed to parse URI: {}", e)))
}

