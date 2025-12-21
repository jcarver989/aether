//! LSP client for communicating with language servers
//!
//! This module provides `LspClient`, which manages the lifecycle of a language server
//! process and handles JSON-RPC communication over stdio.

use std::collections::HashMap;
use std::io::BufReader;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use lsp_types::{
    ClientCapabilities, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, GeneralClientCapabilities, InitializeParams, InitializeResult,
    InitializedParams, ProgressParams, PublishDiagnosticsClientCapabilities,
    PublishDiagnosticsParams, TextDocumentClientCapabilities, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
    WindowClientCapabilities,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot};

use super::error::{LspError, Result};
use super::transport::{
    read_message, write_notification, write_request, JsonRpcNotification,
    JsonRpcRequest,
};

/// Callback type for handling notifications from the server
type NotificationHandler = Box<dyn Fn(&str, serde_json::Value) + Send + Sync>;

/// Notifications received from the LSP server
#[derive(Debug, Clone)]
pub enum LspNotification {
    /// Diagnostics published for a document (errors, warnings, etc.)
    Diagnostics(PublishDiagnosticsParams),
    /// Progress updates (indexing, loading workspace, etc.)
    Progress(ProgressParams),
}

/// An LSP client that manages a language server process
///
/// The client handles:
/// - Spawning and managing the server process
/// - JSON-RPC request/response correlation
/// - Notification dispatch (e.g., diagnostics)
/// - LSP lifecycle (initialize, initialized, shutdown)
pub struct LspClient {
    /// The language server process
    process: Child,
    /// Counter for generating unique request IDs
    request_id: AtomicI64,
    /// Pending requests waiting for responses
    pending_requests: Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>>,
    /// Channel to receive notifications (diagnostics, progress, etc.)
    notification_rx: mpsc::Receiver<LspNotification>,
    /// Sender for notifications (kept alive to prevent channel closure)
    #[allow(dead_code)]
    notification_tx: mpsc::Sender<LspNotification>,
    /// Custom notification handlers
    notification_handlers: Arc<Mutex<Vec<NotificationHandler>>>,
    /// Whether the client has been initialized
    initialized: bool,
    /// Handle to the message reader thread
    _reader_handle: thread::JoinHandle<()>,
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
    /// let client = LspClient::spawn("rust-analyzer", &[])?;
    /// ```
    pub fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        let mut process = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| LspError::Transport("Failed to capture stdout".into()))?;

        let (notification_tx, notification_rx) = mpsc::channel(200);
        let pending_requests: Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let notification_handlers: Arc<Mutex<Vec<NotificationHandler>>> =
            Arc::new(Mutex::new(Vec::new()));

        // Clone for the reader thread
        let pending_requests_clone = Arc::clone(&pending_requests);
        let notification_tx_clone = notification_tx.clone();
        let notification_handlers_clone = Arc::clone(&notification_handlers);

        // Spawn a thread to read messages from the server
        let reader_handle = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);

            loop {
                match read_message(&mut reader) {
                    Ok(message) => {
                        if message.is_response() {
                            // Handle response to a request
                            if let Some(id) = message.id {
                                let sender = pending_requests_clone.lock().unwrap().remove(&id);
                                if let Some(sender) = sender {
                                    if let Some(error) = message.error {
                                        // Send error as a JSON value for the caller to handle
                                        let _ = sender.send(serde_json::json!({
                                            "__lsp_error": true,
                                            "code": error.code,
                                            "message": error.message
                                        }));
                                    } else if let Some(result) = message.result {
                                        let _ = sender.send(result);
                                    }
                                }
                            }
                        } else if message.is_notification() {
                            // Handle notification from server
                            let method = message.method.as_deref().unwrap_or("");
                            let params = message.params.unwrap_or(serde_json::Value::Null);

                            // Route notifications through the unified channel
                            if method == "textDocument/publishDiagnostics" {
                                if let Ok(diag_params) =
                                    serde_json::from_value::<PublishDiagnosticsParams>(params.clone())
                                {
                                    let _ = notification_tx_clone
                                        .blocking_send(LspNotification::Diagnostics(diag_params));
                                }
                            } else if method == "$/progress" {
                                if let Ok(progress_params) =
                                    serde_json::from_value::<ProgressParams>(params.clone())
                                {
                                    let _ = notification_tx_clone
                                        .blocking_send(LspNotification::Progress(progress_params));
                                }
                            }

                            // Call custom notification handlers
                            let handlers = notification_handlers_clone.lock().unwrap();
                            for handler in handlers.iter() {
                                handler(method, params.clone());
                            }
                        }
                    }
                    Err(LspError::Transport(msg)) if msg.contains("closed connection") => {
                        // Server shut down, exit the loop
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Error reading LSP message: {}", e);
                    }
                }
            }
        });

        Ok(Self {
            process,
            request_id: AtomicI64::new(1),
            pending_requests,
            notification_rx,
            notification_tx,
            notification_handlers,
            initialized: false,
            _reader_handle: reader_handle,
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
                    // Declare support for server-initiated progress
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
    pub fn did_open(&mut self, uri: Uri, language_id: &str, text: String) -> Result<()> {
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
    pub fn did_change(&mut self, uri: Uri, version: i32, text: String) -> Result<()> {
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
    pub fn did_save(&mut self, uri: Uri) -> Result<()> {
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
            // Check if we have any notifications
            match tokio::time::timeout(
                std::time::Duration::from_millis(500),
                self.recv_notification(),
            )
            .await
            {
                Ok(Some(LspNotification::Progress(progress))) => {
                    let token = match &progress.token {
                        lsp_types::NumberOrString::Number(n) => n.to_string(),
                        lsp_types::NumberOrString::String(s) => s.clone(),
                    };

                    match progress.value {
                        lsp_types::ProgressParamsValue::WorkDone(work_done) => {
                            match work_done {
                                lsp_types::WorkDoneProgress::Begin(_) => {
                                    active_tokens.insert(token);
                                }
                                lsp_types::WorkDoneProgress::End(_) => {
                                    active_tokens.remove(&token);
                                }
                                lsp_types::WorkDoneProgress::Report(_) => {
                                    // Still in progress
                                }
                            }
                        }
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

    /// Add a custom notification handler
    ///
    /// The handler will be called for all notifications from the server.
    pub fn on_notification<F>(&mut self, handler: F)
    where
        F: Fn(&str, serde_json::Value) + Send + Sync + 'static,
    {
        self.notification_handlers
            .lock()
            .unwrap()
            .push(Box::new(handler));
    }

    /// Shutdown the language server gracefully
    ///
    /// This sends the `shutdown` request followed by the `exit` notification,
    /// then waits for the process to terminate.
    pub async fn shutdown(&mut self) -> Result<()> {
        // Send shutdown request
        let _: serde_json::Value = self.send_request("shutdown", ()).await?;

        // Send exit notification
        self.send_notification("exit", ())?;

        // Wait for process to exit
        self.process.wait()?;

        Ok(())
    }

    /// Check if the client has been initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Send a request and wait for the response
    async fn send_request<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R> {
        let id = self.next_request_id();
        let request = JsonRpcRequest::new(id, method, params);

        // Create a oneshot channel for the response
        let (tx, rx) = oneshot::channel();

        // Register the pending request
        self.pending_requests.lock().unwrap().insert(id, tx);

        // Write the request
        let stdin = self
            .process
            .stdin
            .as_mut()
            .ok_or_else(|| LspError::Transport("stdin not available".into()))?;
        write_request(stdin, &request)?;

        // Wait for the response
        let response = rx
            .await
            .map_err(|_| LspError::Transport("Response channel closed".into()))?;

        // Check for error response
        if response.get("__lsp_error").is_some() {
            let code = response["code"].as_i64().unwrap_or(-1) as i32;
            let message = response["message"]
                .as_str()
                .unwrap_or("Unknown error")
                .to_string();
            return Err(LspError::ServerError { code, message });
        }

        // Deserialize the result
        serde_json::from_value(response).map_err(LspError::from)
    }

    /// Send a notification (no response expected)
    fn send_notification<P: Serialize>(&mut self, method: &str, params: P) -> Result<()> {
        let notification = JsonRpcNotification::new(method, params);

        let stdin = self
            .process
            .stdin
            .as_mut()
            .ok_or_else(|| LspError::Transport("stdin not available".into()))?;
        write_notification(stdin, &notification)
    }

    /// Generate the next unique request ID
    fn next_request_id(&self) -> i64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Try to kill the process if it's still running
        let _ = self.process.kill();
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
            absolute_path
                .to_string_lossy()
                .replace('\\', "/")
        )
    } else {
        format!("file://{}", absolute_path.display())
    };

    uri_str
        .parse()
        .map_err(|e| LspError::Transport(format!("Failed to parse URI: {}", e)))
}
