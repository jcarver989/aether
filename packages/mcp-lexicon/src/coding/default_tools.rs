use std::collections::HashMap;
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
    WriteFileResponse, edit_file_contents, execute_command, list_files, read_background_bash,
    read_file_contents, tools_trait::CodingTools, write_file_contents,
};

/// Request to query the diagnostics cache
type DiagnosticsQuery = oneshot::Sender<HashMap<Uri, Vec<Diagnostic>>>;

/// Default implementation that uses local filesystem operations.
///
/// This is the standard behavior for CodingMcp when running outside
/// of an ACP context. Handles LSP synchronization internally when
/// an LSP client is configured.
#[derive(Debug)]
pub struct DefaultCodingTools {
    /// Notification sender for the LSP client (clone as needed)
    lsp_tx: Option<NotificationSender>,
    /// Track document versions for LSP (URI -> version)
    document_versions: Mutex<HashMap<Uri, i32>>,
    /// Channel to query diagnostics from the listener task
    diagnostics_query_tx: Option<mpsc::Sender<DiagnosticsQuery>>,
    /// Handle to the notification listener task (kept alive)
    _listener_task: Option<JoinHandle<()>>,
}

impl Default for DefaultCodingTools {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultCodingTools {
    /// Create a new DefaultCodingTools instance without LSP
    pub fn new() -> Self {
        Self {
            lsp_tx: None,
            document_versions: Mutex::new(HashMap::new()),
            diagnostics_query_tx: None,
            _listener_task: None,
        }
    }

    /// Create a new DefaultCodingTools instance with LSP integration
    ///
    /// The caller retains ownership of `LspClient` and is responsible for
    /// calling `client.shutdown()` when done. This struct only needs the
    /// notification channels.
    pub fn with_lsp(mut self, tx: NotificationSender, rx: NotificationReceiver) -> Self {
        let (query_tx, query_rx) = mpsc::channel(16);
        self.lsp_tx = Some(tx);
        self.diagnostics_query_tx = Some(query_tx);
        self._listener_task = Some(spawn(run_cache_actor(rx, query_rx)));
        self
    }

    /// Notify the LSP that a file was opened
    fn notify_lsp_did_open(&self, file_path: &str, content: &str) {
        let Some(tx) = &self.lsp_tx else { return };
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
        let _ = tx.try_send(ClientNotification::TextDocumentOpened(params));
    }

    /// Notify the LSP that a file was changed
    fn notify_lsp_did_change(&self, file_path: &str, content: &str) {
        let Some(tx) = &self.lsp_tx else { return };
        let Ok(uri) = path_to_uri(std::path::Path::new(file_path)) else {
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
        let _ = tx.try_send(ClientNotification::TextDocumentChanged(params));
    }
}

impl CodingTools for DefaultCodingTools {
    async fn read_file(&self, args: ReadFileArgs) -> Result<ReadFileResult, String> {
        let file_path = args.file_path.clone();
        let result = read_file_contents(args)
            .await
            .map_err(|e| format!("Read file error: {e}"))?;

        // Notify LSP that file was opened
        self.notify_lsp_did_open(&file_path, &result.raw_content);

        Ok(result)
    }

    async fn write_file(&self, args: WriteFileArgs) -> Result<WriteFileResponse, String> {
        let file_path = args.file_path.clone();
        let content = args.content.clone();

        let result = write_file_contents(args)
            .await
            .map_err(|e| format!("Write file error: {e}"))?;

        // Notify LSP that file changed
        self.notify_lsp_did_change(&file_path, &content);

        Ok(result)
    }

    async fn edit_file(&self, args: EditFileArgs) -> Result<EditFileResponse, String> {
        let file_path = args.file_path.clone();

        let result = edit_file_contents(args)
            .await
            .map_err(|e| format!("Edit file error: {e}"))?;

        // Notify LSP that file changed
        self.notify_lsp_did_change(&file_path, &result.content);

        Ok(result)
    }

    async fn list_files(&self, args: ListFilesArgs) -> Result<ListFilesResult, String> {
        list_files(args)
            .await
            .map_err(|e| format!("List files error: {e}"))
    }

    async fn bash(&self, args: BashInput) -> Result<BashResult, String> {
        execute_command(args)
            .await
            .map_err(|e| format!("Bash command error: {e}"))
    }

    async fn read_background_bash(
        &self,
        handle: BackgroundProcessHandle,
        filter: Option<String>,
    ) -> Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), String> {
        read_background_bash(handle, filter)
            .await
            .map_err(|e| format!("Failed to get output: {e}"))
    }

    async fn get_lsp_diagnostics(&self) -> Result<HashMap<Uri, Vec<Diagnostic>>, String> {
        let Some(tx) = &self.diagnostics_query_tx else {
            return Err("LSP not configured for this CodingTools instance".to_string());
        };

        let (response_tx, response_rx) = oneshot::channel();

        if tx.send(response_tx).await.is_err() {
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
    use std::path::Path;

    #[test]
    fn test_language_id_from_path() {
        assert_eq!(
            LanguageId::from_path(Path::new("file.rs")),
            LanguageId::Rust
        );
        assert_eq!(
            LanguageId::from_path(Path::new("main.py")),
            LanguageId::Python
        );
        assert_eq!(
            LanguageId::from_path(Path::new("script.js")),
            LanguageId::JavaScript
        );
        assert_eq!(
            LanguageId::from_path(Path::new("component.tsx")),
            LanguageId::TypeScriptReact
        );
        assert_eq!(LanguageId::from_path(Path::new("main.go")), LanguageId::Go);
        assert_eq!(
            LanguageId::from_path(Path::new("config.yaml")),
            LanguageId::Yaml
        );
        assert_eq!(
            LanguageId::from_path(Path::new("config.yml")),
            LanguageId::Yaml
        );
        assert_eq!(
            LanguageId::from_path(Path::new("unknown.xyz")),
            LanguageId::PlainText
        );
        assert_eq!(
            LanguageId::from_path(Path::new("noextension")),
            LanguageId::PlainText
        );
    }

    #[test]
    fn test_language_id_to_string() {
        assert_eq!(LanguageId::Rust.to_string(), "rust");
        assert_eq!(LanguageId::Python.to_string(), "python");
        assert_eq!(LanguageId::JavaScript.to_string(), "javascript");
        assert_eq!(LanguageId::PlainText.to_string(), "plaintext");
    }
}
