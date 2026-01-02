use super::error::{LspError, Result};
use super::transport::{
    ClientNotification, ClientRequest, ParsedMessage, ParsedNotification, read_message,
    send_notification, send_request,
};
use lsp_types::{
    CallHierarchyClientCapabilities, CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams,
    CallHierarchyItem, CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams, ClientCapabilities, DocumentSymbolClientCapabilities,
    DocumentSymbolParams, DocumentSymbolResponse, DynamicRegistrationClientCapabilities,
    GeneralClientCapabilities, GotoCapability, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverClientCapabilities, HoverParams, InitializeParams, Location, MarkupKind, Position,
    ProgressParams, PublishDiagnosticsClientCapabilities, PublishDiagnosticsParams,
    ReferenceContext, ReferenceParams, SymbolInformation, TextDocumentClientCapabilities,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri, WindowClientCapabilities,
    WorkspaceSymbolParams,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Stdio, id};
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
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
    GotoImplementation {
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
    DocumentSymbol {
        id: i64,
        params: DocumentSymbolParams,
        response_tx: oneshot::Sender<Result<DocumentSymbolResponse>>,
    },
    PrepareCallHierarchy {
        id: i64,
        params: CallHierarchyPrepareParams,
        response_tx: oneshot::Sender<Result<Vec<CallHierarchyItem>>>,
    },
    IncomingCalls {
        id: i64,
        params: CallHierarchyIncomingCallsParams,
        response_tx: oneshot::Sender<Result<Vec<CallHierarchyIncomingCall>>>,
    },
    OutgoingCalls {
        id: i64,
        params: CallHierarchyOutgoingCallsParams,
        response_tx: oneshot::Sender<Result<Vec<CallHierarchyOutgoingCall>>>,
    },
}

/// Pending response channels, keyed by request ID
enum PendingResponse {
    Initialize(oneshot::Sender<Result<()>>),
    Shutdown(oneshot::Sender<Result<()>>),
    GotoDefinition(oneshot::Sender<Result<GotoDefinitionResponse>>),
    GotoImplementation(oneshot::Sender<Result<GotoDefinitionResponse>>),
    FindReferences(oneshot::Sender<Result<Vec<Location>>>),
    Hover(oneshot::Sender<Result<Option<Hover>>>),
    WorkspaceSymbol(oneshot::Sender<Result<Vec<SymbolInformation>>>),
    DocumentSymbol(oneshot::Sender<Result<DocumentSymbolResponse>>),
    PrepareCallHierarchy(oneshot::Sender<Result<Vec<CallHierarchyItem>>>),
    IncomingCalls(oneshot::Sender<Result<Vec<CallHierarchyIncomingCall>>>),
    OutgoingCalls(oneshot::Sender<Result<Vec<CallHierarchyOutgoingCall>>>),
}

impl PendingResponse {
    /// Send an error to the appropriate typed channel
    fn send_error(self, err: LspError) {
        match self {
            Self::Initialize(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::Shutdown(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::GotoDefinition(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::GotoImplementation(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::FindReferences(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::Hover(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::WorkspaceSymbol(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::DocumentSymbol(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::PrepareCallHierarchy(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::IncomingCalls(tx) => {
                let _ = tx.send(Err(err));
            }
            Self::OutgoingCalls(tx) => {
                let _ = tx.send(Err(err));
            }
        }
    }

    /// Send a successful response to the appropriate typed channel
    fn send_response(self, value: Value) {
        match self {
            Self::Initialize(tx) => {
                let _ = tx.send(Ok(()));
            }
            Self::Shutdown(tx) => {
                let _ = tx.send(Ok(()));
            }
            Self::GotoDefinition(tx) => {
                let result =
                    parse_lsp_response(value, GotoDefinitionResponse::Array(vec![]), "definition");
                let _ = tx.send(result);
            }
            Self::GotoImplementation(tx) => {
                let result = parse_lsp_response(
                    value,
                    GotoDefinitionResponse::Array(vec![]),
                    "implementation",
                );
                let _ = tx.send(result);
            }
            Self::FindReferences(tx) => {
                let result = parse_lsp_response(value, vec![], "references");
                let _ = tx.send(result);
            }
            Self::Hover(tx) => {
                // Hover uses Option<Hover>, needs explicit .map(Some)
                let result = if value.is_null() {
                    Ok(None)
                } else {
                    serde_json::from_value::<Hover>(value)
                        .map(Some)
                        .map_err(|e| {
                            LspError::InvalidMessage(format!(
                                "Failed to parse hover response: {}",
                                e
                            ))
                        })
                };
                let _ = tx.send(result);
            }
            Self::WorkspaceSymbol(tx) => {
                let result = parse_lsp_response(value, vec![], "workspace symbol");
                let _ = tx.send(result);
            }
            Self::DocumentSymbol(tx) => {
                let result = parse_lsp_response(
                    value,
                    DocumentSymbolResponse::Flat(vec![]),
                    "document symbol",
                );
                let _ = tx.send(result);
            }
            Self::PrepareCallHierarchy(tx) => {
                let result = parse_lsp_response(value, vec![], "prepare call hierarchy");
                let _ = tx.send(result);
            }
            Self::IncomingCalls(tx) => {
                let result = parse_lsp_response(value, vec![], "incoming calls");
                let _ = tx.send(result);
            }
            Self::OutgoingCalls(tx) => {
                let result = parse_lsp_response(value, vec![], "outgoing calls");
                let _ = tx.send(result);
            }
        }
    }
}

/// Parse an LSP JSON response with null-handling
fn parse_lsp_response<T: serde::de::DeserializeOwned>(
    value: Value,
    null_default: T,
    type_name: &str,
) -> Result<T> {
    if value.is_null() {
        Ok(null_default)
    } else {
        serde_json::from_value(value).map_err(|e| {
            LspError::InvalidMessage(format!("Failed to parse {} response: {}", type_name, e))
        })
    }
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
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| LspError::Transport("Failed to capture stdin".into()))?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| LspError::Transport("Failed to capture stdout".into()))?;

        let stderr = process
            .stderr
            .take()
            .ok_or_else(|| LspError::Transport("Failed to capture stderr".into()))?;

        let (request_tx, request_rx) = channel(100);
        let (client_notif_tx, client_notif_rx) = channel(100);
        let (server_notif_tx, server_notif_rx) = channel(200);

        // Spawn stderr reader task
        spawn_stderr_reader(stderr);

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

        // Declare support for document symbol capability
        let document_symbol_capability = DocumentSymbolClientCapabilities {
            hierarchical_document_symbol_support: Some(true),
            ..Default::default()
        };

        // Declare support for call hierarchy capability
        let call_hierarchy_capability = CallHierarchyClientCapabilities {
            dynamic_registration: Some(false),
        };

        let text_document_capabilities = TextDocumentClientCapabilities {
            publish_diagnostics: Some(pub_diagnostic_capabilities),
            definition: Some(definition_capability),
            implementation: Some(definition_capability),
            references: Some(references_capability),
            hover: Some(hover_capability),
            document_symbol: Some(document_symbol_capability),
            call_hierarchy: Some(call_hierarchy_capability),
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

        response_rx.await.map_err(|_| LspError::response_closed())?
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

        response_rx.await.map_err(|_| LspError::response_closed())?
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

        response_rx.await.map_err(|_| LspError::response_closed())?
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

        response_rx.await.map_err(|_| LspError::response_closed())?
    }

    /// Go to the implementation of an interface/trait method
    ///
    /// # Arguments
    /// * `uri` - The URI of the document
    /// * `line` - Line number (0-indexed)
    /// * `character` - Character offset (0-indexed)
    ///
    /// # Returns
    /// The implementation response, which may be a single location, multiple locations,
    /// or location links depending on the server.
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

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::GotoImplementation {
                id,
                params,
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx.await.map_err(|_| LspError::response_closed())?
    }

    /// Get all symbols in a document
    ///
    /// # Arguments
    /// * `uri` - The URI of the document
    ///
    /// # Returns
    /// Document symbol response which can be either flat or hierarchical symbols.
    pub async fn document_symbol(&self, uri: Uri) -> Result<DocumentSymbolResponse> {
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::DocumentSymbol {
                id,
                params,
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx.await.map_err(|_| LspError::response_closed())?
    }

    /// Prepare call hierarchy at a position
    ///
    /// # Arguments
    /// * `uri` - The URI of the document
    /// * `line` - Line number (0-indexed)
    /// * `character` - Character offset (0-indexed)
    ///
    /// # Returns
    /// A list of call hierarchy items at the position (usually 0 or 1 items).
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

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::PrepareCallHierarchy {
                id,
                params,
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx.await.map_err(|_| LspError::response_closed())?
    }

    /// Get incoming calls for a call hierarchy item
    ///
    /// # Arguments
    /// * `item` - The call hierarchy item to get incoming calls for
    ///
    /// # Returns
    /// A list of incoming calls (functions/methods that call this item).
    pub async fn incoming_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyIncomingCall>> {
        let params = CallHierarchyIncomingCallsParams {
            item,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::IncomingCalls {
                id,
                params,
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx.await.map_err(|_| LspError::response_closed())?
    }

    /// Get outgoing calls for a call hierarchy item
    ///
    /// # Arguments
    /// * `item` - The call hierarchy item to get outgoing calls for
    ///
    /// # Returns
    /// A list of outgoing calls (functions/methods that this item calls).
    pub async fn outgoing_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyOutgoingCall>> {
        let params = CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(Request::OutgoingCalls {
                id,
                params,
                response_tx,
            })
            .await
            .map_err(|_| LspError::handler_closed())?;

        response_rx.await.map_err(|_| LspError::response_closed())?
    }

    /// Generate the next unique request ID
    fn next_request_id(&self) -> i64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }
}

/// The handler task that owns stdin, stdout, and pending requests map
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
                    Request::GotoImplementation { id, params, response_tx } => (
                        ClientRequest::GotoImplementation(id, params),
                        PendingResponse::GotoImplementation(response_tx),
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
                    Request::DocumentSymbol { id, params, response_tx } => (
                        ClientRequest::DocumentSymbol(id, params),
                        PendingResponse::DocumentSymbol(response_tx),
                    ),
                    Request::PrepareCallHierarchy { id, params, response_tx } => (
                        ClientRequest::PrepareCallHierarchy(id, params),
                        PendingResponse::PrepareCallHierarchy(response_tx),
                    ),
                    Request::IncomingCalls { id, params, response_tx } => (
                        ClientRequest::IncomingCalls(id, params),
                        PendingResponse::IncomingCalls(response_tx),
                    ),
                    Request::OutgoingCalls { id, params, response_tx } => (
                        ClientRequest::OutgoingCalls(id, params),
                        PendingResponse::OutgoingCalls(response_tx),
                    ),
                };

                let id = client_request.id();
                pending.insert(id, pending_response);
                if let Err(e) = send_request(&mut stdin, &client_request).await
                    && let Some(p) = pending.remove(&id) {
                        p.send_error(e);
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
        p.send_error(LspError::Transport("Handler task closed".into()));
    }

    // Explicitly kill the process to avoid orphaned LSP processes.
    let _ = process.kill().await;
}

/// Spawn a background task to read and filter LSP stderr
///
/// This task reads stderr from the LSP process and logs lines at DEBUG level,
/// filtering out common non-critical warnings.
fn spawn_stderr_reader(stderr: ChildStderr) -> JoinHandle<()> {
    spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            // Filter out common non-critical warnings
            if !line.trim().is_empty() && !is_filtered_stderr(&line) {
                tracing::debug!("LSP stderr: {}", line.trim());
            }
        }
    })
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
        Some(e) => pending_response.send_error(LspError::ServerError {
            code: e.code,
            message: e.message,
        }),
        None => pending_response.send_response(resp.result.unwrap_or(Value::Null)),
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

/// Check if stderr line should be filtered out (not logged)
///
/// This filters out common non-critical warnings from LSP servers:
/// - rust-analyzer's "notify error" about missing config files
/// - Other benign warnings that don't indicate real problems
fn is_filtered_stderr(line: &str) -> bool {
    let lower = line.to_lowercase();

    // Filter rust-analyzer's notify error about missing config
    if lower.contains("notify error") && lower.contains("no path was found") {
        return true;
    }

    // Filter other non-critical warnings
    if lower.contains("warn")
        && (lower.contains("application support")
            || lower.contains("config") && lower.contains("not found"))
    {
        return true;
    }

    false
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_filtered_stderr_filters_notify_error() {
        // The exact message from rust-analyzer about missing config
        let msg = "2025-12-27T13:30:41.405378-08:00  WARN notify error: No path was found. about [\"/Users/jj/Library/Application Support/rust-analyzer/rust-analyzer.toml\"]";
        assert!(
            is_filtered_stderr(msg),
            "Should filter rust-analyzer notify error"
        );
    }

    #[test]
    fn test_is_filtered_stderr_filters_config_warnings() {
        // Config-related warnings with "application support"
        assert!(is_filtered_stderr(
            "WARN: Config file not found at /Users/foo/Library/Application Support/rust-analyzer/config.toml"
        ));
        assert!(is_filtered_stderr(
            "WARN: application support directory missing"
        ));
        // Note: simple "config not found" warnings are not filtered - only specific patterns
    }

    #[test]
    fn test_is_filtered_stderr_does_not_filter_errors() {
        // Actual errors should not be filtered
        assert!(!is_filtered_stderr("error: Failed to parse file"));
        assert!(!is_filtered_stderr("ERROR: Connection refused"));
        assert!(!is_filtered_stderr("Error: Invalid token"));
    }

    #[test]
    fn test_is_filtered_stderr_does_not_filter_normal_output() {
        // Normal output should not be filtered
        assert!(!is_filtered_stderr("Loading workspace..."));
        assert!(!is_filtered_stderr("Indexing 123 files"));
        assert!(!is_filtered_stderr("info: Server started"));
    }

    #[test]
    fn test_is_filtered_stderr_case_insensitive() {
        // Should work regardless of case
        assert!(is_filtered_stderr("WARN notify error: No path was found"));
        assert!(is_filtered_stderr("warn Notify Error: No path was found"));
    }

    #[test]
    fn test_is_filtered_stderr_empty_lines() {
        // Empty lines are filtered separately but should return false here
        assert!(!is_filtered_stderr(""));
        assert!(!is_filtered_stderr("   "));
    }
}
