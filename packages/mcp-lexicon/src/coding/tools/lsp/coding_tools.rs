use lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    GotoDefinitionResponse, Hover, Location, SymbolInformation, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::spawn;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[cfg(test)]
use crate::coding::tools::bash::BashOutput;
use crate::coding::lsp::{
    ClientNotification, LanguageId, LspClient, NotificationReceiver, NotificationSender,
    ServerNotification, path_to_uri,
};
use crate::coding::tools::bash::{BackgroundProcessHandle, BashInput, BashResult};
use crate::coding::tools::edit_file::{EditFileArgs, EditFileResponse};
use crate::coding::tools::list_files::{ListFilesArgs, ListFilesResult};
use crate::coding::tools::bash::ReadBackgroundBashOutput;
use crate::coding::tools::read_file::{ReadFileArgs, ReadFileResult};
use crate::coding::tools::write_file::{WriteFileArgs, WriteFileResponse};
use crate::coding::tools_trait::CodingTools;

/// Request to query the diagnostics cache (keyed by URI string)
type DiagnosticsQuery = oneshot::Sender<HashMap<String, Vec<Diagnostic>>>;

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
/// enabling diagnostics (errors, warnings) and code intelligence (goto definition,
/// find references, hover) to be available to the agent.
///
/// # Usage
///
/// ```ignore
/// // Spawn LSP-enabled coding tools for a Rust project
/// let tools = LspCodingTools::spawn(
///     DefaultCodingTools::new(),
///     "rust-analyzer",
///     &[],
///     &project_path,
/// ).await?;
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
    /// The LSP client for making requests (definition, references, hover)
    lsp_client: Arc<LspClient>,
}

impl<T: CodingTools> Debug for LspCodingTools<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspCodingTools")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<T: CodingTools> LspCodingTools<T> {
    /// Spawn an LSP server and wrap the given CodingTools implementation with LSP integration.
    ///
    /// # Arguments
    /// * `inner` - The underlying CodingTools implementation to wrap
    /// * `command` - The LSP server command (e.g., "rust-analyzer")
    /// * `args` - Arguments to pass to the LSP server
    /// * `project_root` - The root directory of the project
    ///
    /// # Returns
    /// A new LspCodingTools instance with full LSP integration, or an error if
    /// the LSP server failed to spawn.
    pub async fn spawn(
        inner: T,
        command: &str,
        args: &[&str],
        project_root: &Path,
    ) -> Result<Self, String> {
        let (lsp_tx, lsp_rx, lsp_client) = LspClient::spawn(command, args, project_root)
            .await
            .map_err(|e| format!("Failed to spawn LSP server '{}': {}", command, e))?;

        let (query_tx, query_rx) = mpsc::channel(16);
        Ok(Self {
            inner,
            lsp_tx,
            open_documents: Mutex::new(HashMap::new()),
            diagnostics_query_tx: query_tx,
            _listener_task: spawn(run_cache_actor(lsp_rx, query_rx)),
            lsp_client: Arc::new(lsp_client),
        })
    }

    /// Create LspCodingTools for testing without a real LSP server.
    ///
    /// This is only available in test builds. The returned instance will have
    /// diagnostics support but LSP requests will fail.
    #[cfg(test)]
    pub(crate) fn new_for_testing(
        inner: T,
        lsp_tx: NotificationSender,
        lsp_rx: NotificationReceiver,
    ) -> Self {
        // We need a real channel for the cache actor
        let (query_tx, query_rx) = mpsc::channel(16);

        // Create a fake client - requests will fail but that's fine for notification tests
        let fake_client = Arc::new(LspClient::new_for_testing(lsp_tx.clone()));

        Self {
            inner,
            lsp_tx,
            open_documents: Mutex::new(HashMap::new()),
            diagnostics_query_tx: query_tx,
            _listener_task: spawn(run_cache_actor(lsp_rx, query_rx)),
            lsp_client: fake_client,
        }
    }

    /// Ensure a document is open with the LSP, sending didOpen if needed.
    /// Returns the current version number (1 if just opened, or incremented if already open).
    fn ensure_open(&self, file_path: &str, content: &str) -> Option<i32> {
        let Ok(uri) = path_to_uri(Path::new(file_path)) else {
            return None;
        };

        {
            let mut docs = self.open_documents.lock().unwrap();
            if let Some(state) = docs.get_mut(&uri) {
                state.version += 1;
                return Some(state.version);
            }
        }

        let language_id = LanguageId::from_path(Path::new(file_path));
        let version = 1;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: language_id.to_string(),
                version,
                text: content.to_string(),
            },
        };

        let _ = self
            .lsp_tx
            .try_send(ClientNotification::TextDocumentOpened(params));

        self.open_documents.lock().unwrap().insert(
            uri,
            DocumentState {
                version,
                language_id,
            },
        );

        Some(version)
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

    /// Ensure a file is open with the LSP and return its content.
    /// If the file is not already open, reads it and sends didOpen.
    async fn ensure_file_open_and_get_content(&self, file_path: &str) -> Result<String, String> {
        // Always read the file to get current content (needed for symbol lookup)
        let result = self
            .inner
            .read_file(ReadFileArgs {
                file_path: file_path.to_string(),
                offset: None,
                limit: None,
            })
            .await?;

        self.ensure_open(file_path, &result.raw_content);
        Ok(result.raw_content)
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

    async fn get_lsp_diagnostics(&self) -> Result<HashMap<String, Vec<Diagnostic>>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        if self.diagnostics_query_tx.send(response_tx).await.is_err() {
            return Err(
                "Failed to query diagnostics cache - listener task may have stopped".to_string(),
            );
        }

        Ok(response_rx.await.unwrap_or_default())
    }

    async fn goto_definition(
        &self,
        file_path: &str,
        symbol: &str,
        line: u32,
    ) -> Result<GotoDefinitionResponse, String> {
        let content = self.ensure_file_open_and_get_content(file_path).await?;
        let column = find_symbol_column(&content, symbol, line)?;

        let uri = path_to_uri(Path::new(file_path))
            .map_err(|e| format!("Failed to convert path to URI: {}", e))?;

        self.lsp_client
            .goto_definition(uri, line - 1, column)
            .await
            .map_err(|e| format!("LSP goto_definition failed: {}", e))
    }

    async fn find_references(
        &self,
        file_path: &str,
        symbol: &str,
        line: u32,
        include_declaration: bool,
    ) -> Result<Vec<Location>, String> {
        let content = self.ensure_file_open_and_get_content(file_path).await?;
        let column = find_symbol_column(&content, symbol, line)?;
        let uri = path_to_uri(Path::new(file_path))
            .map_err(|e| format!("Failed to convert path to URI: {}", e))?;

        self.lsp_client
            .find_references(uri, line - 1, column, include_declaration)
            .await
            .map_err(|e| format!("LSP find_references failed: {}", e))
    }

    async fn hover(
        &self,
        file_path: &str,
        symbol: &str,
        line: u32,
    ) -> Result<Option<Hover>, String> {
        let content = self.ensure_file_open_and_get_content(file_path).await?;
        let column = find_symbol_column(&content, symbol, line)?;
        let uri = path_to_uri(Path::new(file_path))
            .map_err(|e| format!("Failed to convert path to URI: {}", e))?;

        self.lsp_client
            .hover(uri, line - 1, column)
            .await
            .map_err(|e| format!("LSP hover failed: {}", e))
    }

    async fn workspace_symbol(&self, query: &str) -> Result<Vec<SymbolInformation>, String> {
        self.lsp_client
            .workspace_symbol(query.to_string())
            .await
            .map_err(|e| format!("LSP workspace_symbol failed: {}", e))
    }
}

/// Find the column position of a symbol on a specific line.
///
/// # Arguments
/// * `content` - The full file content
/// * `symbol` - The symbol name to find
/// * `line` - Line number (1-indexed)
///
/// # Returns
/// The column position (0-indexed) of the first occurrence of the symbol on that line.
fn find_symbol_column(content: &str, symbol: &str, line: u32) -> Result<u32, String> {
    let line_idx = line
        .checked_sub(1)
        .ok_or_else(|| "Line number must be >= 1".to_string())?;

    let line_content = content
        .lines()
        .nth(line_idx as usize)
        .ok_or_else(|| format!("Line {} not found in file", line))?;

    // Find the symbol on the line - match word boundaries to avoid partial matches
    let mut search_start = 0;
    while let Some(pos) = line_content[search_start..].find(symbol) {
        let abs_pos = search_start + pos;
        let before_ok = abs_pos == 0
            || !line_content[..abs_pos]
                .chars()
                .last()
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false);
        let after_ok = abs_pos + symbol.len() >= line_content.len()
            || !line_content[abs_pos + symbol.len()..]
                .chars()
                .next()
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false);

        if before_ok && after_ok {
            return Ok(abs_pos as u32);
        }
        search_start = abs_pos + 1;
    }

    Err(format!("Symbol '{}' not found on line {}", symbol, line))
}

/// Actor task that owns the diagnostics cache and responds to queries
async fn run_cache_actor(
    mut notification_rx: NotificationReceiver,
    mut query_rx: mpsc::Receiver<DiagnosticsQuery>,
) {
    let mut cache: HashMap<String, Vec<Diagnostic>> = HashMap::new();

    loop {
        tokio::select! {
            Some(notification) = notification_rx.recv() => {
                if let ServerNotification::Diagnostics(params) = notification {
                    cache.insert(params.uri.to_string(), params.diagnostics);
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
        let tools = LspCodingTools::new_for_testing(inner, client_tx, server_rx);

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
        let tools = LspCodingTools::new_for_testing(inner, client_tx, server_rx);

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
        let tools = LspCodingTools::new_for_testing(inner, client_tx, server_rx);

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
        let tools = LspCodingTools::new_for_testing(inner, client_tx, server_rx);

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
        let tools = LspCodingTools::new_for_testing(inner, client_tx, server_rx);

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

    #[test]
    fn test_find_symbol_column_basic() {
        let content = "fn main() {\n    let x = HashMap::new();\n}";
        // "HashMap" is on line 2, starting at column 12 (0-indexed)
        assert_eq!(find_symbol_column(content, "HashMap", 2).unwrap(), 12);
    }

    #[test]
    fn test_find_symbol_column_first_line() {
        let content = "use std::collections::HashMap;";
        assert_eq!(find_symbol_column(content, "HashMap", 1).unwrap(), 22);
    }

    #[test]
    fn test_find_symbol_column_word_boundary() {
        // Should not match "HashMapExtra" when looking for "HashMap"
        let content = "let x = HashMapExtra::new();";
        assert!(find_symbol_column(content, "HashMap", 1).is_err());
    }

    #[test]
    fn test_find_symbol_column_word_boundary_prefix() {
        // Should not match "MyHashMap" when looking for "HashMap"
        let content = "let x = MyHashMap::new();";
        assert!(find_symbol_column(content, "HashMap", 1).is_err());
    }

    #[test]
    fn test_find_symbol_column_underscore_boundary() {
        // Underscores are part of identifiers, so "hash_map" should not match "hash"
        let content = "let hash_map = 1;";
        assert!(find_symbol_column(content, "hash", 1).is_err());
    }

    #[test]
    fn test_find_symbol_column_not_found() {
        let content = "fn main() {}";
        let result = find_symbol_column(content, "HashMap", 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found on line"));
    }

    #[test]
    fn test_find_symbol_column_line_out_of_range() {
        let content = "fn main() {}";
        let result = find_symbol_column(content, "main", 99);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found in file"));
    }

    #[test]
    fn test_find_symbol_column_zero_line() {
        let content = "fn main() {}";
        let result = find_symbol_column(content, "main", 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be >= 1"));
    }

    #[test]
    fn test_find_symbol_column_multiple_on_line() {
        // When there are multiple occurrences on a line, we return the first one
        let content = "let x = foo + foo;";
        assert_eq!(find_symbol_column(content, "foo", 1).unwrap(), 8);
    }
}
