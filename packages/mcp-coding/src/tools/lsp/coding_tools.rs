//! Multi-language LSP support for CodingTools
//!
//! This module provides `LspCodingTools`, a wrapper that extends any `CodingTools`
//! implementation with LSP support. It uses an `LspRegistry` to lazily spawn and manage
//! multiple LSP servers based on file type.

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Diagnostic,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentSymbolResponse, GotoDefinitionResponse, Hover, Location, SymbolInformation,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem, Uri,
    VersionedTextDocumentIdentifier,
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use aether_lspd::LspClient;

use crate::error::CodingError;
use crate::lsp::LanguageId;
use crate::lsp::common::{path_to_uri, uri_to_path};
use crate::lsp::registry::LspRegistry;
use crate::tools::bash::ReadBackgroundBashOutput;
use crate::tools::bash::{BackgroundProcessHandle, BashInput, BashResult};
use crate::tools::edit_file::{EditFileArgs, EditFileResponse};
use crate::tools::list_files::{ListFilesArgs, ListFilesResult};
use crate::tools::read_file::{ReadFileArgs, ReadFileResult};
use crate::tools::write_file::{WriteFileArgs, WriteFileResponse};
use crate::tools_trait::CodingTools;

/// State for a document tracked by the LSP wrapper
#[derive(Debug, Clone)]
struct DocumentState {
    /// Current version number (incremented on each change)
    version: i32,
    /// Whether an LSP is handling this document
    has_lsp: bool,
}

/// A CodingTools wrapper that provides multi-language LSP support.
///
/// This wrapper intercepts file operations and notifies the appropriate language server,
/// enabling diagnostics (errors, warnings) and code intelligence (goto definition,
/// find references, hover) to be available to the agent.
///
/// LSP servers are spawned lazily on first file access for each language type,
/// which provides efficient resource usage and fast startup.
///
/// # Supported Languages (by default)
///
/// - Rust (`rust-analyzer`)
/// - TypeScript/JavaScript (`typescript-language-server`)
/// - Python (`pyright-langserver`)
/// - Go (`gopls`)
/// - C/C++ (`clangd`)
///
/// # Example
///
/// ```ignore
/// use mcp_coding::default_tools::DefaultCodingTools;
/// use mcp_coding::tools::lsp::LspCodingTools;
///
/// let tools = LspCodingTools::new(
///     DefaultCodingTools::new(),
///     PathBuf::from("/path/to/project"),
/// );
///
/// // When reading a .rs file, rust-analyzer will be spawned lazily
/// let result = tools.read_file(ReadFileArgs {
///     file_path: "/path/to/project/src/main.rs".to_string(),
///     offset: None,
///     limit: None,
/// }).await?;
/// ```
pub struct LspCodingTools<T: CodingTools> {
    inner: T,
    registry: Arc<LspRegistry>,
    /// Track open documents with their state (URI -> DocumentState)
    open_documents: Mutex<HashMap<Uri, DocumentState>>,
}

impl<T: CodingTools> Debug for LspCodingTools<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspCodingTools")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<T: CodingTools> LspCodingTools<T> {
    /// Create LSP-enabled coding tools for a project.
    ///
    /// Uses the daemon's built-in configurations for common languages:
    /// - rust-analyzer for Rust
    /// - typescript-language-server for TypeScript/JavaScript
    /// - pyright-langserver for Python
    /// - gopls for Go
    /// - clangd for C/C++
    ///
    /// LSP servers for detected project languages are spawned immediately in the background.
    pub fn new(inner: T, root_path: PathBuf) -> Self {
        let registry = Arc::new(LspRegistry::new(root_path));

        // Spawn LSPs for detected project languages in background
        // This allows LSPs like rust-analyzer to start indexing immediately
        let registry_clone = Arc::clone(&registry);
        tokio::spawn(async move {
            registry_clone.spawn_project_lsps().await;
        });

        Self {
            inner,
            registry,
            open_documents: Mutex::new(HashMap::new()),
        }
    }

    /// Ensure a document is open with its LSP, sending didOpen if needed.
    /// Returns the current version number (1 if just opened, or incremented if already open).
    async fn ensure_open(&self, file_path: &str, content: &str) -> Option<(i32, Arc<LspClient>)> {
        let path = Path::new(file_path);
        let Ok(uri) = path_to_uri(path) else {
            return None;
        };

        let existing_state = match self.open_documents.lock().unwrap().get_mut(&uri) {
            Some(state) => {
                state.version += 1;
                Some((state.version, state.has_lsp))
            }
            None => None,
        };

        match existing_state {
            Some((version, true)) => {
                return self.registry.get_or_spawn(path).await.map(|h| (version, h));
            }
            Some(_) => return None,
            None => {}
        }

        let client = self.registry.get_or_spawn(path).await;
        let language_id = LanguageId::from_path(path);
        let version = 1;

        if let Some(ref client) = client {
            let params = DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: language_id.as_str().to_string(),
                    version,
                    text: content.to_string(),
                },
            };

            let _ = client.notify_opened(params).await;
        }

        let has_lsp = client.is_some();
        self.open_documents
            .lock()
            .unwrap()
            .insert(uri, DocumentState { version, has_lsp });

        client.map(|c| (version, c))
    }

    /// Notify the LSP that a document was changed (requires document to be open)
    async fn notify_did_change(&self, file_path: &str, content: &str, version: i32) {
        let path = Path::new(file_path);
        let Ok(uri) = path_to_uri(path) else {
            return;
        };

        if let Some(client) = self.registry.get_or_spawn(path).await {
            let params = DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri, version },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: content.to_string(),
                }],
            };
            let _ = client.notify_changed(params).await;
        }
    }

    /// Notify the LSP that a document was saved
    async fn notify_did_save(&self, file_path: &str, content: Option<&str>) {
        let path = Path::new(file_path);
        let Ok(uri) = path_to_uri(path) else {
            return;
        };

        if let Some(client) = self.registry.get_or_spawn(path).await {
            let params = DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri },
                text: content.map(|s| s.to_string()),
            };
            let _ = client.notify_saved(params).await;
        }
    }

    /// Ensure a file is open with the LSP and return its content.
    async fn ensure_file_open_and_get_content(
        &self,
        file_path: &str,
    ) -> Result<String, CodingError> {
        let result = self
            .inner
            .read_file(ReadFileArgs {
                file_path: file_path.to_string(),
                offset: None,
                limit: None,
            })
            .await?;

        self.ensure_open(file_path, &result.raw_content).await;
        Ok(result.raw_content)
    }
}

impl<T: CodingTools> CodingTools for LspCodingTools<T> {
    async fn read_file(&self, args: ReadFileArgs) -> Result<ReadFileResult, CodingError> {
        let file_path = args.file_path.clone();
        let result = self.inner.read_file(args).await?;
        // Ensure document is open (sends didOpen if first time)
        self.ensure_open(&file_path, &result.raw_content).await;
        Ok(result)
    }

    async fn write_file(&self, args: WriteFileArgs) -> Result<WriteFileResponse, CodingError> {
        let file_path = args.file_path.clone();
        let content = args.content.clone();
        let result = self.inner.write_file(args).await?;

        if let Some((version, _)) = self.ensure_open(&file_path, &content).await {
            if version > 1 {
                self.notify_did_change(&file_path, &content, version).await;
            }
            self.notify_did_save(&file_path, Some(&content)).await;
        }

        Ok(result)
    }

    async fn edit_file(&self, args: EditFileArgs) -> Result<EditFileResponse, CodingError> {
        let file_path = args.file_path.clone();
        let result = self.inner.edit_file(args).await?;

        if let Some((version, _)) = self.ensure_open(&file_path, &result.content).await {
            if version > 1 {
                self.notify_did_change(&file_path, &result.content, version)
                    .await;
            }
            self.notify_did_save(&file_path, Some(&result.content))
                .await;
        }

        Ok(result)
    }

    async fn list_files(&self, args: ListFilesArgs) -> Result<ListFilesResult, CodingError> {
        self.inner.list_files(args).await
    }

    async fn bash(&self, args: BashInput) -> Result<BashResult, CodingError> {
        self.inner.bash(args).await
    }

    async fn read_background_bash(
        &self,
        handle: BackgroundProcessHandle,
        filter: Option<String>,
    ) -> Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), CodingError> {
        self.inner.read_background_bash(handle, filter).await
    }

    async fn get_lsp_diagnostics(&self) -> Result<HashMap<String, Vec<Diagnostic>>, CodingError> {
        let mut result = HashMap::new();

        // Query diagnostics from all active LSP clients
        for client in self.registry.active_clients().await {
            if let Ok(params_list) = client.get_diagnostics(None).await {
                for params in params_list {
                    let file_path = uri_to_path(&params.uri);
                    result
                        .entry(file_path)
                        .or_insert_with(Vec::new)
                        .extend(params.diagnostics);
                }
            }
        }

        Ok(result)
    }

    async fn goto_definition(
        &self,
        file_path: &str,
        symbol: &str,
        line: u32,
    ) -> Result<GotoDefinitionResponse, CodingError> {
        let content = self.ensure_file_open_and_get_content(file_path).await?;
        let column = find_symbol_column(&content, symbol, line)?;

        let uri = path_to_uri(Path::new(file_path)).map_err(CodingError::from)?;

        let client = self
            .registry
            .get_or_spawn(Path::new(file_path))
            .await
            .ok_or_else(|| {
                CodingError::NotConfigured("No LSP configured for this file type".to_string())
            })?;

        client
            .goto_definition(uri, line - 1, column)
            .await
            .map_err(CodingError::from)
    }

    async fn find_references(
        &self,
        file_path: &str,
        symbol: &str,
        line: u32,
        include_declaration: bool,
    ) -> Result<Vec<Location>, CodingError> {
        let content = self.ensure_file_open_and_get_content(file_path).await?;
        let column = find_symbol_column(&content, symbol, line)?;

        let uri = path_to_uri(Path::new(file_path)).map_err(CodingError::from)?;

        let client = self
            .registry
            .get_or_spawn(Path::new(file_path))
            .await
            .ok_or_else(|| {
                CodingError::NotConfigured("No LSP configured for this file type".to_string())
            })?;

        client
            .find_references(uri, line - 1, column, include_declaration)
            .await
            .map_err(CodingError::from)
    }

    async fn hover(
        &self,
        file_path: &str,
        symbol: &str,
        line: u32,
    ) -> Result<Option<Hover>, CodingError> {
        let content = self.ensure_file_open_and_get_content(file_path).await?;
        let column = find_symbol_column(&content, symbol, line)?;

        let uri = path_to_uri(Path::new(file_path)).map_err(CodingError::from)?;

        let client = self
            .registry
            .get_or_spawn(Path::new(file_path))
            .await
            .ok_or_else(|| {
                CodingError::NotConfigured("No LSP configured for this file type".to_string())
            })?;

        client
            .hover(uri, line - 1, column)
            .await
            .map_err(CodingError::from)
    }

    async fn workspace_symbol(&self, query: &str) -> Result<Vec<SymbolInformation>, CodingError> {
        let mut all_symbols = Vec::new();
        for client in self.registry.active_clients().await {
            if let Ok(symbols) = client.workspace_symbol(query.to_string()).await {
                all_symbols.extend(symbols);
            }
        }
        Ok(all_symbols)
    }

    async fn goto_implementation(
        &self,
        file_path: &str,
        symbol: &str,
        line: u32,
    ) -> Result<GotoDefinitionResponse, CodingError> {
        let content = self.ensure_file_open_and_get_content(file_path).await?;
        let column = find_symbol_column(&content, symbol, line)?;

        let uri = path_to_uri(Path::new(file_path)).map_err(CodingError::from)?;

        let client = self
            .registry
            .get_or_spawn(Path::new(file_path))
            .await
            .ok_or_else(|| {
                CodingError::NotConfigured("No LSP configured for this file type".to_string())
            })?;

        client
            .goto_implementation(uri, line - 1, column)
            .await
            .map_err(CodingError::from)
    }

    async fn document_symbol(
        &self,
        file_path: &str,
    ) -> Result<DocumentSymbolResponse, CodingError> {
        self.ensure_file_open_and_get_content(file_path).await?;

        let uri = path_to_uri(Path::new(file_path)).map_err(CodingError::from)?;

        let client = self
            .registry
            .get_or_spawn(Path::new(file_path))
            .await
            .ok_or_else(|| {
                CodingError::NotConfigured("No LSP configured for this file type".to_string())
            })?;

        client.document_symbol(uri).await.map_err(CodingError::from)
    }

    async fn prepare_call_hierarchy(
        &self,
        file_path: &str,
        symbol: &str,
        line: u32,
    ) -> Result<Vec<CallHierarchyItem>, CodingError> {
        let content = self.ensure_file_open_and_get_content(file_path).await?;
        let column = find_symbol_column(&content, symbol, line)?;

        let uri = path_to_uri(Path::new(file_path)).map_err(CodingError::from)?;

        let client = self
            .registry
            .get_or_spawn(Path::new(file_path))
            .await
            .ok_or_else(|| {
                CodingError::NotConfigured("No LSP configured for this file type".to_string())
            })?;

        client
            .prepare_call_hierarchy(uri, line - 1, column)
            .await
            .map_err(CodingError::from)
    }

    async fn incoming_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyIncomingCall>, CodingError> {
        let file_path = uri_to_path(&item.uri);

        let client = self
            .registry
            .get_or_spawn(Path::new(&file_path))
            .await
            .ok_or_else(|| {
                CodingError::NotConfigured("No LSP configured for this file type".to_string())
            })?;

        client.incoming_calls(item).await.map_err(CodingError::from)
    }

    async fn outgoing_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyOutgoingCall>, CodingError> {
        let file_path = uri_to_path(&item.uri);

        let client = self
            .registry
            .get_or_spawn(Path::new(&file_path))
            .await
            .ok_or_else(|| {
                CodingError::NotConfigured("No LSP configured for this file type".to_string())
            })?;

        client.outgoing_calls(item).await.map_err(CodingError::from)
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
fn find_symbol_column(content: &str, symbol: &str, line: u32) -> Result<u32, CodingError> {
    let line_idx = line
        .checked_sub(1)
        .ok_or_else(|| CodingError::NotConfigured("Line number must be >= 1".to_string()))?;

    let line_content = content
        .lines()
        .nth(line_idx as usize)
        .ok_or_else(|| CodingError::NotConfigured(format!("Line {} not found in file", line)))?;

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

    Err(CodingError::NotConfigured(format!(
        "Symbol '{}' not found on line {}",
        symbol, line
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not found on line")
        );
    }

    #[test]
    fn test_find_symbol_column_line_out_of_range() {
        let content = "fn main() {}";
        let result = find_symbol_column(content, "main", 99);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not found in file")
        );
    }

    #[test]
    fn test_find_symbol_column_zero_line() {
        let content = "fn main() {}";
        let result = find_symbol_column(content, "main", 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be >= 1"));
    }

    #[test]
    fn test_find_symbol_column_multiple_on_line() {
        // When there are multiple occurrences on a line, we return the first one
        let content = "let x = foo + foo;";
        assert_eq!(find_symbol_column(content, "foo", 1).unwrap(), 8);
    }
}
