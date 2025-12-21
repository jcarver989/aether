//! LSP client for communicating with language servers
//!
//! This module provides `LspClient`, which manages the lifecycle of a language server
//! process and handles JSON-RPC communication over stdio.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};

use lsp_types::{
    ClientCapabilities, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, GeneralClientCapabilities, InitializeParams, InitializeResult,
    InitializedParams, ProgressParams, PublishDiagnosticsClientCapabilities,
    PublishDiagnosticsParams, TextDocumentClientCapabilities, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
    WindowClientCapabilities,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::error::{LspError, Result};
use super::transport::{
    JsonRpcNotification, JsonRpcRequest, read_message, write_notification, write_request,
};

/// Notifications received from the LSP server
#[derive(Debug, Clone)]
pub enum LspNotification {
    /// Diagnostics published for a document (errors, warnings, etc.)
    Diagnostics(PublishDiagnosticsParams),
    /// Progress updates (indexing, loading workspace, etc.)
    Progress(ProgressParams),
}

/// Commands sent to the handler task
enum LspCommand {
    SendRequest {
        id: i64,
        method: String,
        params: Value,
        response_tx: oneshot::Sender<Result<Value>>,
    },
    SendNotification {
        method: String,
        params: Value,
    },
}

/// An LSP client that manages a language server process
///
/// The client handles:
/// - Spawning and managing the server process
/// - JSON-RPC request/response correlation
/// - Notification dispatch (e.g., diagnostics)
/// - LSP lifecycle (initialize, initialized, shutdown)
pub struct LspClient {
    /// Counter for generating unique request IDs
    request_id: AtomicI64,
    /// Channel to send commands to the handler task
    command_tx: mpsc::Sender<LspCommand>,
    /// Channel to receive notifications (diagnostics, progress, etc.)
    notification_rx: mpsc::Receiver<LspNotification>,
    /// Whether the client has been initialized
    initialized: bool,
    /// Handle to the handler task
    _task_handle: JoinHandle<()>,
}

impl LspClient {
    /// Spawn a new language server process
    ///
    /// # Arguments
    /// * `command` - The command to run (e.g., "rust-analyzer")
    /// * `args` - Arguments to pass to the command
    ///
    /// # Example
    /// ```ignore
    /// let client = LspClient::spawn("rust-analyzer", &[]).await?;
    /// ```
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self> {
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

        let (command_tx, command_rx) = mpsc::channel(100);
        let (notification_tx, notification_rx) = mpsc::channel(200);

        let task_handle = tokio::spawn(run_handler(
            command_rx,
            stdin,
            stdout,
            notification_tx,
            process,
        ));

        Ok(Self {
            request_id: AtomicI64::new(1),
            command_tx,
            notification_rx,
            initialized: false,
            _task_handle: task_handle,
        })
    }

    /// Initialize the language server
    ///
    /// This must be called before using any other LSP functionality.
    /// It sends the `initialize` request and `initialized` notification.
    ///
    /// # Arguments
    /// * `root_path` - The root directory of the project
    pub async fn initialize(&mut self, root_path: &Path) -> Result<InitializeResult> {
        let root_uri = path_to_uri(root_path)?;

        let params = InitializeParams {
            process_id: Some(std::process::id()),
            #[allow(deprecated)]
            root_uri: Some(root_uri.clone()),
            capabilities: ClientCapabilities {
                general: Some(GeneralClientCapabilities {
                    ..Default::default()
                }),
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..Default::default()
                }),
                text_document: Some(TextDocumentClientCapabilities {
                    publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                        related_information: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let result: InitializeResult = self.send_request("initialize", params).await?;

        // Send initialized notification
        self.send_notification("initialized", InitializedParams {})?;
        self.initialized = true;

        Ok(result)
    }

    /// Open a text document in the language server
    ///
    /// This notifies the server about a new file being opened for editing.
    ///
    /// # Arguments
    /// * `uri` - The file URI
    /// * `language_id` - The language identifier (e.g., "rust", "python")
    /// * `text` - The file contents
    pub fn did_open(&self, uri: Uri, language_id: &str, text: String) -> Result<()> {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version: 1,
                text,
            },
        };
        self.send_notification("textDocument/didOpen", params)
    }

    /// Notify the server that a document has changed
    ///
    /// This sends the full new content of the file (full sync mode).
    ///
    /// # Arguments
    /// * `uri` - The file URI
    /// * `version` - The new document version (should be incremented each change)
    /// * `text` - The new file contents
    pub fn did_change(&self, uri: Uri, version: i32, text: String) -> Result<()> {
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text,
            }],
        };
        self.send_notification("textDocument/didChange", params)
    }

    /// Notify the server that a document was saved
    ///
    /// # Arguments
    /// * `uri` - The file URI
    pub fn did_save(&self, uri: Uri) -> Result<()> {
        let params = DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: None,
        };
        self.send_notification("textDocument/didSave", params)
    }

    /// Receive the next notification from the server
    ///
    /// Returns the next [`LspNotification`] received from the server.
    /// Returns `None` if the channel is closed.
    pub async fn recv_notification(&mut self) -> Option<LspNotification> {
        self.notification_rx.recv().await
    }

    /// Try to receive a notification without blocking
    ///
    /// Returns `Some(notification)` if available, `None` otherwise.
    pub fn try_recv_notification(&mut self) -> Option<LspNotification> {
        self.notification_rx.try_recv().ok()
    }

    /// Drain all pending notifications
    ///
    /// Returns all notifications that have been received but not yet consumed.
    pub fn drain_notifications(&mut self) -> Vec<LspNotification> {
        let mut notifications = Vec::new();
        while let Some(notification) = self.try_recv_notification() {
            notifications.push(notification);
        }
        notifications
    }

    /// Wait for the server to finish indexing/loading the workspace
    ///
    /// This waits for all in-progress work to complete by tracking $/progress notifications.
    /// Returns when no progress tokens have active work.
    ///
    /// Note: Non-progress notifications (like diagnostics) received during this wait are discarded.
    pub async fn wait_for_indexing(&mut self) {
        use std::collections::HashSet;

        let mut active_tokens: HashSet<String> = HashSet::new();

        loop {
            match tokio::time::timeout(
                std::time::Duration::from_millis(500),
                self.recv_notification(),
            )
            .await
            {
                Ok(Some(LspNotification::Progress(progress))) => {
                    use lsp_types::NumberOrString::{Number, String};
                    let token = match progress.token {
                        Number(n) => n.to_string(),
                        String(s) => s,
                    };
                    let lsp_types::ProgressParamsValue::WorkDone(work_done) = progress.value;

                    match work_done {
                        lsp_types::WorkDoneProgress::Begin(_) => {
                            active_tokens.insert(token);
                        }
                        lsp_types::WorkDoneProgress::End(_) => {
                            active_tokens.remove(&token);
                        }
                        lsp_types::WorkDoneProgress::Report(_) => {}
                    }
                }
                Ok(Some(_)) => {
                    // Ignore non-progress notifications during indexing wait
                }
                Ok(None) => break, // Channel closed
                Err(_) => {
                    // Timeout - if no active tokens, we're done
                    if active_tokens.is_empty() {
                        break;
                    }
                }
            }
        }
    }

    /// Shutdown the language server gracefully
    ///
    /// This sends the `shutdown` request followed by the `exit` notification,
    /// then waits for the process to terminate.
    pub async fn shutdown(&mut self) -> Result<()> {
        // Send shutdown request
        let _: Value = self.send_request("shutdown", ()).await?;

        // Send exit notification
        self.send_notification("exit", ())?;

        Ok(())
    }

    /// Check if the client has been initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Send a request and wait for the response
    pub async fn send_request<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R> {
        let id = self.next_request_id();
        let (response_tx, response_rx) = oneshot::channel();

        self.command_tx
            .send(LspCommand::SendRequest {
                id,
                method: method.to_string(),
                params: serde_json::to_value(&params)?,
                response_tx,
            })
            .await
            .map_err(|_| LspError::Transport("Handler task closed".into()))?;

        let result = response_rx
            .await
            .map_err(|_| LspError::Transport("Response channel closed".into()))??;

        serde_json::from_value(result).map_err(LspError::from)
    }

    /// Send a notification (no response expected)
    fn send_notification<P: Serialize>(&self, method: &str, params: P) -> Result<()> {
        self.command_tx
            .try_send(LspCommand::SendNotification {
                method: method.to_string(),
                params: serde_json::to_value(&params)?,
            })
            .map_err(|_| LspError::Transport("Handler task closed".into()))
    }

    /// Generate the next unique request ID
    fn next_request_id(&self) -> i64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }
}

/// The handler task that owns stdin, stdout, and the pending requests map
async fn run_handler(
    mut command_rx: mpsc::Receiver<LspCommand>,
    mut stdin: ChildStdin,
    stdout: ChildStdout,
    notification_tx: mpsc::Sender<LspNotification>,
    mut process: Child,
) {
    let mut reader = BufReader::new(stdout);
    let mut pending: HashMap<i64, oneshot::Sender<Result<Value>>> = HashMap::new();

    loop {
        tokio::select! {
            // Handle outgoing commands
            Some(cmd) = command_rx.recv() => {
                match cmd {
                    LspCommand::SendRequest { id, method, params, response_tx } => {
                        pending.insert(id, response_tx);
                        let request = JsonRpcRequest::new(id, &method, params);
                        if let Err(e) = write_request(&mut stdin, &request).await {
                            // If write fails, notify the caller
                            if let Some(tx) = pending.remove(&id) {
                                let _ = tx.send(Err(e));
                            }
                        }
                    }
                    LspCommand::SendNotification { method, params } => {
                        let notification = JsonRpcNotification::new(&method, params);
                        let _ = write_notification(&mut stdin, &notification).await;
                    }
                }
            }

            // Handle incoming LSP messages
            msg = read_message(&mut reader) => {
                match msg {
                    Ok(msg) if msg.is_response() => {
                        if let Some(tx) = msg.id.and_then(|id| pending.remove(&id)) {
                            let result = msg.error.map_or_else(
                                || Ok(msg.result.unwrap_or(Value::Null)),
                                |e| Err(LspError::ServerError { code: e.code, message: e.message }),
                            );
                            let _ = tx.send(result);
                        }
                    }
                    Ok(msg) if msg.is_notification() => {
                        let method = msg.method.as_deref().unwrap_or("");
                        let params = msg.params.unwrap_or(Value::Null);
                        route_notification(method, params, &notification_tx);
                    }
                    Ok(_) => {
                        // Unknown message type, ignore
                    }
                    Err(LspError::Transport(ref s)) if s.contains("closed") => {
                        // Server closed connection, exit
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Error reading LSP message: {}", e);
                    }
                }
            }

            // Handle process exit
            _ = process.wait() => {
                break;
            }
        }
    }

    // Notify any remaining pending requests that the handler is closing
    for (_, tx) in pending {
        let _ = tx.send(Err(LspError::Transport("Handler task closed".into())));
    }
}

/// Route a notification to the appropriate channel
fn route_notification(
    method: &str,
    params: Value,
    notification_tx: &mpsc::Sender<LspNotification>,
) {
    match method {
        "textDocument/publishDiagnostics" => {
            if let Ok(diag_params) = serde_json::from_value::<PublishDiagnosticsParams>(params) {
                let _ = notification_tx.try_send(LspNotification::Diagnostics(diag_params));
            }
        }
        "$/progress" => {
            if let Ok(progress_params) = serde_json::from_value::<ProgressParams>(params) {
                let _ = notification_tx.try_send(LspNotification::Progress(progress_params));
            }
        }
        _ => {
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
