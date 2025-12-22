use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::sync::Mutex;

use lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    TextDocumentContentChangeEvent, TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
};
use tokio::spawn;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

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
pub struct LspAwareCodingTools<T: CodingTools> {
    inner: T,
    /// Notification sender for the LSP client
    lsp_tx: NotificationSender,
    /// Track document versions for LSP (URI -> version)
    document_versions: Mutex<HashMap<Uri, i32>>,
    /// Channel to query diagnostics from the listener task
    diagnostics_query_tx: mpsc::Sender<DiagnosticsQuery>,
    /// Handle to the notification listener task (kept alive)
    _listener_task: JoinHandle<()>,
}

impl<T: CodingTools> Debug for LspAwareCodingTools<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspAwareCodingTools")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<T: CodingTools> LspAwareCodingTools<T> {
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
            document_versions: Mutex::new(HashMap::new()),
            diagnostics_query_tx: query_tx,
            _listener_task: spawn(run_cache_actor(lsp_rx, query_rx)),
        }
    }

    /// Notify the LSP that a file was opened
    fn notify_lsp_did_open(&self, file_path: &str, content: &str) {
        let Ok(uri) = path_to_uri(Path::new(file_path)) else {
            return;
        };

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: LanguageId::from_path(Path::new(file_path)).to_string(),
                version: 1,
                text: content.to_string(),
            },
        };
        let _ = self.lsp_tx.try_send(ClientNotification::TextDocumentOpened(params));
    }

    /// Notify the LSP that a file was changed
    fn notify_lsp_did_change(&self, file_path: &str, content: &str) {
        let Ok(uri) = path_to_uri(Path::new(file_path)) else {
            return;
        };
        let version = {
            let mut versions = self.document_versions.lock().unwrap();
            let version = versions.entry(uri.clone()).or_insert(0);
            *version += 1;
            *version
        };
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: content.to_string(),
            }],
        };
        let _ = self.lsp_tx.try_send(ClientNotification::TextDocumentChanged(params));
    }
}

impl<T: CodingTools> CodingTools for LspAwareCodingTools<T> {
    async fn read_file(&self, args: ReadFileArgs) -> Result<ReadFileResult, String> {
        let file_path = args.file_path.clone();
        let result = self.inner.read_file(args).await?;

        // Notify LSP that file was opened
        self.notify_lsp_did_open(&file_path, &result.raw_content);

        Ok(result)
    }

    async fn write_file(&self, args: WriteFileArgs) -> Result<WriteFileResponse, String> {
        let file_path = args.file_path.clone();
        let content = args.content.clone();

        let result = self.inner.write_file(args).await?;

        // Notify LSP that file changed
        self.notify_lsp_did_change(&file_path, &content);

        Ok(result)
    }

    async fn edit_file(&self, args: EditFileArgs) -> Result<EditFileResponse, String> {
        let file_path = args.file_path.clone();

        let result = self.inner.edit_file(args).await?;

        // Notify LSP that file changed
        self.notify_lsp_did_change(&file_path, &result.content);

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

