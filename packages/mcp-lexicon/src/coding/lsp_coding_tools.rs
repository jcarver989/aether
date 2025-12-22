use lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem, Uri,
    VersionedTextDocumentIdentifier,
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::sync::Mutex;
use tokio::spawn;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[cfg(test)]
use super::BashOutput;
use super::lsp::{
    ClientNotification, LanguageId, NotificationReceiver, NotificationSender, ServerNotification,
    path_to_uri,
};
use super::{
    BackgroundProcessHandle, BashInput, BashResult, EditFileArgs, EditFileResponse, ListFilesArgs,
    ListFilesResult, ReadBackgroundBashOutput, ReadFileArgs, ReadFileResult, WriteFileArgs,
    WriteFileResponse, tools_trait::CodingTools,
};

/// Request to query the diagnostics cache
type DiagnosticsQuery = oneshot::Sender<HashMap<Uri, Vec<Diagnostic>>>;

/// State for a document tracked by the LSP wrapper
#[derive(Debug, Clone)]
struct DocumentState {
    /// Current version number (incremented on each change)
    version: i32,
    /// Language ID for this document (detected from extension)
    #[allow(dead_code)]
    language_id: LanguageId,
}

/// A wrapper that adds LSP integration to any CodingTools implementation.
///
/// This wrapper intercepts file operations and notifies the language server,
/// enabling diagnostics (errors, warnings) to be tracked for the agent.
///
/// # Usage
///
/// ```ignore
/// // Wrap DefaultCodingTools with LSP
/// let (tx, rx, client) = LspClient::spawn("rust-analyzer", &[], &project_path).await?;
/// let tools = LspAwareCodingTools::new(DefaultCodingTools::new(), tx, rx);
/// ```
pub struct LspCodingTools<T: CodingTools> {
    inner: T,
    /// Notification sender for the LSP client
    lsp_tx: NotificationSender,
    /// Track open documents with their state (URI -> DocumentState)
    open_documents: Mutex<HashMap<Uri, DocumentState>>,
    /// Channel to query diagnostics from the listener task
    diagnostics_query_tx: mpsc::Sender<DiagnosticsQuery>,
    /// Handle to the notification listener task (kept alive)
    _listener_task: JoinHandle<()>,
}

impl<T: CodingTools> Debug for LspCodingTools<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspAwareCodingTools")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<T: CodingTools> LspCodingTools<T> {
    /// Create a new LspAwareCodingTools wrapping the given implementation.
    ///
    /// The caller retains ownership of `LspClient` and is responsible for
    /// calling `client.shutdown()` when done. This struct only needs the
    /// notification channels.
    pub fn new(inner: T, lsp_tx: NotificationSender, lsp_rx: NotificationReceiver) -> Self {
        let (query_tx, query_rx) = mpsc::channel(16);
        Self {
            inner,
            lsp_tx,
            open_documents: Mutex::new(HashMap::new()),
            diagnostics_query_tx: query_tx,
            _listener_task: spawn(run_cache_actor(lsp_rx, query_rx)),
        }
    }

    /// Ensure a document is open with the LSP, sending didOpen if needed.
    /// Returns the current version number (1 if just opened, or incremented if already open).
    fn ensure_open(&self, file_path: &str, content: &str) -> Option<i32> {
        let Ok(uri) = path_to_uri(Path::new(file_path)) else {
            return None;
        };

        let mut docs = self.open_documents.lock().unwrap();

        if let Some(state) = docs.get_mut(&uri) {
            // Already open, just increment and return version
            state.version += 1;
            Some(state.version)
        } else {
            // Not open yet, send didOpen
            let language_id = LanguageId::from_path(Path::new(file_path));
            let params = DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: language_id.to_string(),
                    version: 1,
                    text: content.to_string(),
                },
            };

            let _ = self
                .lsp_tx
                .try_send(ClientNotification::TextDocumentOpened(params));

            docs.insert(
                uri,
                DocumentState {
                    version: 1,
                    language_id,
                },
            );
            Some(1)
        }
    }

    /// Notify the LSP that a document was changed (requires document to be open)
    fn notify_did_change(&self, file_path: &str, content: &str, version: i32) {
        let Ok(uri) = path_to_uri(Path::new(file_path)) else {
            return;
        };

        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: content.to_string(),
            }],
        };
        let _ = self
            .lsp_tx
            .try_send(ClientNotification::TextDocumentChanged(params));
    }

    /// Notify the LSP that a document was saved
    fn notify_did_save(&self, file_path: &str, content: Option<&str>) {
        let Ok(uri) = path_to_uri(Path::new(file_path)) else {
            return;
        };

        let params = DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: content.map(|s| s.to_string()),
        };
        let _ = self
            .lsp_tx
            .try_send(ClientNotification::TextDocumentSaved(params));
    }
}

impl<T: CodingTools> CodingTools for LspCodingTools<T> {
    async fn read_file(&self, args: ReadFileArgs) -> Result<ReadFileResult, String> {
        let file_path = args.file_path.clone();
        let result = self.inner.read_file(args).await?;
        // Ensure document is open (sends didOpen if first time, idempotent otherwise)
        self.ensure_open(&file_path, &result.raw_content);
        Ok(result)
    }

    async fn write_file(&self, args: WriteFileArgs) -> Result<WriteFileResponse, String> {
        let file_path = args.file_path.clone();
        let content = args.content.clone();
        let result = self.inner.write_file(args).await?;
        if let Some(version) = self.ensure_open(&file_path, &content) {
            if version > 1 {
                self.notify_did_change(&file_path, &content, version);
            }

            self.notify_did_save(&file_path, Some(&content));
        }

        Ok(result)
    }

    async fn edit_file(&self, args: EditFileArgs) -> Result<EditFileResponse, String> {
        let file_path = args.file_path.clone();
        let result = self.inner.edit_file(args).await?;

        if let Some(version) = self.ensure_open(&file_path, &result.content) {
            if version > 1 {
                self.notify_did_change(&file_path, &result.content, version);
            }

            self.notify_did_save(&file_path, Some(&result.content));
        }

        Ok(result)
    }

    async fn list_files(&self, args: ListFilesArgs) -> Result<ListFilesResult, String> {
        self.inner.list_files(args).await
    }

    async fn bash(&self, args: BashInput) -> Result<BashResult, String> {
        self.inner.bash(args).await
    }

    async fn read_background_bash(
        &self,
        handle: BackgroundProcessHandle,
        filter: Option<String>,
    ) -> Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), String> {
        self.inner.read_background_bash(handle, filter).await
    }

    async fn get_lsp_diagnostics(&self) -> Result<HashMap<Uri, Vec<Diagnostic>>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        if self.diagnostics_query_tx.send(response_tx).await.is_err() {
            return Err(
                "Failed to query diagnostics cache - listener task may have stopped".to_string(),
            );
        }

        Ok(response_rx.await.unwrap_or_default())
    }
}

/// Actor task that owns the diagnostics cache and responds to queries
async fn run_cache_actor(
    mut notification_rx: NotificationReceiver,
    mut query_rx: mpsc::Receiver<DiagnosticsQuery>,
) {
    let mut cache: HashMap<Uri, Vec<Diagnostic>> = HashMap::new();

    loop {
        tokio::select! {
            Some(notification) = notification_rx.recv() => {
                if let ServerNotification::Diagnostics(params) = notification {
                    cache.insert(params.uri, params.diagnostics);
                }
            }
            Some(response_tx) = query_rx.recv() => {
                let _ = response_tx.send(cache.clone());
            }
            else => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::channel;

    /// A fake CodingTools implementation for testing
    #[derive(Debug)]
    struct FakeCodingTools {
        /// Content to return from read_file
        read_content: String,
        /// Content to return from edit_file
        edit_content: String,
    }

    impl FakeCodingTools {
        fn new(read_content: &str, edit_content: &str) -> Self {
            Self {
                read_content: read_content.to_string(),
                edit_content: edit_content.to_string(),
            }
        }
    }

    impl CodingTools for FakeCodingTools {
        async fn read_file(&self, args: ReadFileArgs) -> Result<ReadFileResult, String> {
            Ok(ReadFileResult {
                status: "success".to_string(),
                file_path: args.file_path,
                content: self.read_content.clone(),
                total_lines: 1,
                lines_shown: 1,
                offset: 0,
                limit: None,
                size: self.read_content.len(),
                raw_content: self.read_content.clone(),
            })
        }

        async fn write_file(&self, args: WriteFileArgs) -> Result<WriteFileResponse, String> {
            Ok(WriteFileResponse {
                message: "File written".to_string(),
                bytes_written: args.content.len(),
                file_path: args.file_path,
            })
        }

        async fn edit_file(&self, args: EditFileArgs) -> Result<EditFileResponse, String> {
            Ok(EditFileResponse {
                status: "edited".to_string(),
                file_path: args.file_path,
                total_lines: 10,
                replacements_made: 1,
                content: self.edit_content.clone(),
            })
        }

        async fn list_files(&self, args: ListFilesArgs) -> Result<ListFilesResult, String> {
            Ok(ListFilesResult {
                status: "success".to_string(),
                directory: args.path.unwrap_or_default(),
                files: vec![],
                total_count: 0,
            })
        }

        async fn bash(&self, _args: BashInput) -> Result<BashResult, String> {
            Ok(BashResult::Completed(BashOutput {
                output: String::new(),
                exit_code: 0,
                killed: None,
                shell_id: None,
            }))
        }

        async fn read_background_bash(
            &self,
            handle: BackgroundProcessHandle,
            _filter: Option<String>,
        ) -> Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), String> {
            Ok((
                ReadBackgroundBashOutput {
                    output: String::new(),
                    status: "completed".to_string(),
                    exit_code: Some(0),
                },
                Some(handle),
            ))
        }
    }

    /// Helper to collect notifications from a channel
    fn collect_notifications(
        rx: &mut mpsc::Receiver<ClientNotification>,
    ) -> Vec<ClientNotification> {
        let mut notifications = Vec::new();
        while let Ok(notif) = rx.try_recv() {
            notifications.push(notif);
        }
        notifications
    }

    #[tokio::test]
    async fn test_read_file_sends_did_open_once() {
        let (client_tx, mut client_rx) = channel(100);
        let (_server_tx, server_rx) = channel(100);

        let inner = FakeCodingTools::new("file content", "");
        let tools = LspCodingTools::new(inner, client_tx, server_rx);

        // First read should send didOpen
        let _ = tools
            .read_file(ReadFileArgs {
                file_path: "/test/file.rs".to_string(),
                offset: None,
                limit: None,
            })
            .await;

        let notifications = collect_notifications(&mut client_rx);
        assert_eq!(notifications.len(), 1);
        assert!(matches!(
            &notifications[0],
            ClientNotification::TextDocumentOpened(params) if params.text_document.version == 1
        ));

        // Second read should NOT send another didOpen (just increments version internally)
        let _ = tools
            .read_file(ReadFileArgs {
                file_path: "/test/file.rs".to_string(),
                offset: None,
                limit: None,
            })
            .await;

        let notifications = collect_notifications(&mut client_rx);
        // No new notifications - file is already open
        assert_eq!(notifications.len(), 0);
    }

    #[tokio::test]
    async fn test_write_file_sends_did_open_and_did_save_for_new_file() {
        let (client_tx, mut client_rx) = channel(100);
        let (_server_tx, server_rx) = channel(100);

        let inner = FakeCodingTools::new("", "");
        let tools = LspCodingTools::new(inner, client_tx, server_rx);

        // Write to a file that hasn't been read (opened) yet
        let _ = tools
            .write_file(WriteFileArgs {
                file_path: "/test/new_file.rs".to_string(),
                content: "new content".to_string(),
            })
            .await;

        let notifications = collect_notifications(&mut client_rx);
        assert_eq!(notifications.len(), 2);

        // First notification should be didOpen
        assert!(matches!(
            &notifications[0],
            ClientNotification::TextDocumentOpened(params) if params.text_document.version == 1
        ));

        // Second notification should be didSave
        assert!(matches!(
            &notifications[1],
            ClientNotification::TextDocumentSaved(_)
        ));
    }

    #[tokio::test]
    async fn test_write_file_sends_did_change_and_did_save_for_open_file() {
        let (client_tx, mut client_rx) = channel(100);
        let (_server_tx, server_rx) = channel(100);

        let inner = FakeCodingTools::new("original content", "");
        let tools = LspCodingTools::new(inner, client_tx, server_rx);

        // First read the file (opens it)
        let _ = tools
            .read_file(ReadFileArgs {
                file_path: "/test/file.rs".to_string(),
                offset: None,
                limit: None,
            })
            .await;

        // Clear the didOpen notification
        let _ = collect_notifications(&mut client_rx);

        // Now write to the file
        let _ = tools
            .write_file(WriteFileArgs {
                file_path: "/test/file.rs".to_string(),
                content: "modified content".to_string(),
            })
            .await;

        let notifications = collect_notifications(&mut client_rx);
        assert_eq!(notifications.len(), 2);

        // First should be didChange with version 2
        assert!(matches!(
            &notifications[0],
            ClientNotification::TextDocumentChanged(params) if params.text_document.version == 2
        ));

        // Second should be didSave
        assert!(matches!(
            &notifications[1],
            ClientNotification::TextDocumentSaved(_)
        ));
    }

    #[tokio::test]
    async fn test_edit_file_sends_did_change_and_did_save() {
        let (client_tx, mut client_rx) = channel(100);
        let (_server_tx, server_rx) = channel(100);

        let inner = FakeCodingTools::new("original content", "edited content");
        let tools = LspCodingTools::new(inner, client_tx, server_rx);

        // First read the file (opens it)
        let _ = tools
            .read_file(ReadFileArgs {
                file_path: "/test/file.rs".to_string(),
                offset: None,
                limit: None,
            })
            .await;

        // Clear the didOpen notification
        let _ = collect_notifications(&mut client_rx);

        // Now edit the file
        let _ = tools
            .edit_file(EditFileArgs {
                file_path: "/test/file.rs".to_string(),
                old_string: "original".to_string(),
                new_string: "edited".to_string(),
                replace_all: false,
            })
            .await;

        let notifications = collect_notifications(&mut client_rx);
        assert_eq!(notifications.len(), 2);

        // First should be didChange with version 2
        assert!(matches!(
            &notifications[0],
            ClientNotification::TextDocumentChanged(params) if params.text_document.version == 2
        ));

        // Second should be didSave
        assert!(matches!(
            &notifications[1],
            ClientNotification::TextDocumentSaved(_)
        ));
    }

    #[tokio::test]
    async fn test_version_increments_correctly() {
        let (client_tx, mut client_rx) = channel(100);
        let (_server_tx, server_rx) = channel(100);

        let inner = FakeCodingTools::new("content", "edited");
        let tools = LspCodingTools::new(inner, client_tx, server_rx);

        // Read file (version 1)
        let _ = tools
            .read_file(ReadFileArgs {
                file_path: "/test/file.rs".to_string(),
                offset: None,
                limit: None,
            })
            .await;

        let notifications = collect_notifications(&mut client_rx);
        assert!(matches!(
            &notifications[0],
            ClientNotification::TextDocumentOpened(params) if params.text_document.version == 1
        ));

        // Edit file (version 2)
        let _ = tools
            .edit_file(EditFileArgs {
                file_path: "/test/file.rs".to_string(),
                old_string: "a".to_string(),
                new_string: "b".to_string(),
                replace_all: false,
            })
            .await;

        let notifications = collect_notifications(&mut client_rx);
        assert!(matches!(
            &notifications[0],
            ClientNotification::TextDocumentChanged(params) if params.text_document.version == 2
        ));

        // Another edit (version 3)
        let _ = tools
            .edit_file(EditFileArgs {
                file_path: "/test/file.rs".to_string(),
                old_string: "b".to_string(),
                new_string: "c".to_string(),
                replace_all: false,
            })
            .await;

        let notifications = collect_notifications(&mut client_rx);
        assert!(matches!(
            &notifications[0],
            ClientNotification::TextDocumentChanged(params) if params.text_document.version == 3
        ));
    }
}
