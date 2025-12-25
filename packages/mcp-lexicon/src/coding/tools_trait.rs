use std::future::Future;
use std::{collections::HashMap, fmt::Debug};

use lsp_types::{Diagnostic, GotoDefinitionResponse, Hover, Location, SymbolInformation};

use super::error::CodingError;
use super::tools::bash::{
    BackgroundProcessHandle, BashInput, BashResult, ReadBackgroundBashOutput,
};
use super::tools::edit_file::{EditFileArgs, EditFileResponse};
use super::tools::find::{FindInput, FindOutput, find_files_by_name};
use super::tools::grep::{GrepInput, GrepOutput, perform_grep};
use super::tools::list_files::{ListFilesArgs, ListFilesResult};
use super::tools::read_file::{ReadFileArgs, ReadFileResult};
use super::tools::write_file::{WriteFileArgs, WriteFileResponse};

/// Trait defining the underlying implementation for coding tool operations.
///
/// This trait allows CodingMcp to be used in different contexts:
/// - DefaultCodingTools: Uses local filesystem (default behavior)
/// - AcpCodingTools: Delegates to ACP client for editor integration
pub trait CodingTools: Send + Sync + Debug {
    /// Read a file's contents
    fn read_file(
        &self,
        args: ReadFileArgs,
    ) -> impl Future<Output = Result<ReadFileResult, CodingError>> + Send;

    /// Write content to a file
    fn write_file(
        &self,
        args: WriteFileArgs,
    ) -> impl Future<Output = Result<WriteFileResponse, CodingError>> + Send;

    /// Edit a file using string replacement
    fn edit_file(
        &self,
        args: EditFileArgs,
    ) -> impl Future<Output = Result<EditFileResponse, CodingError>> + Send;

    /// List files in a directory
    fn list_files(
        &self,
        args: ListFilesArgs,
    ) -> impl Future<Output = Result<ListFilesResult, CodingError>> + Send;

    /// Execute a bash command
    fn bash(&self, args: BashInput)
    -> impl Future<Output = Result<BashResult, CodingError>> + Send;

    /// Read output from a background bash process
    fn read_background_bash(
        &self,
        handle: BackgroundProcessHandle,
        filter: Option<String>,
    ) -> impl Future<
        Output = Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), CodingError>,
    > + Send;

    /// Search file contents using regex patterns.
    ///
    /// Searches for a pattern in files within a directory, with support for
    /// glob filtering, file type filtering, and various output modes.
    fn grep(
        &self,
        args: GrepInput,
    ) -> impl Future<Output = Result<GrepOutput, CodingError>> + Send {
        async move { perform_grep(args).await.map_err(CodingError::from) }
    }

    /// Find files by name using glob patterns.
    ///
    /// Searches for files matching a glob pattern within a directory.
    fn find(
        &self,
        args: FindInput,
    ) -> impl Future<Output = Result<FindOutput, CodingError>> + Send {
        async move { find_files_by_name(args).await.map_err(CodingError::from) }
    }

    /// Get all cached LSP diagnostics (errors, warnings, etc.).
    ///
    /// Returns diagnostics keyed by file URI string.
    /// Returns an error if LSP is not configured for this instance.
    fn get_lsp_diagnostics(
        &self,
    ) -> impl Future<Output = Result<HashMap<String, Vec<Diagnostic>>, CodingError>> + Send {
        async { Ok(HashMap::new()) }
    }

    /// Go to the definition of a symbol.
    ///
    /// # Arguments
    /// * `file_path` - The path to the file containing the symbol
    /// * `symbol` - The symbol name to look up (e.g., "LspClient", "spawn", "HashMap")
    /// * `line` - Line number where the symbol appears (1-indexed, as shown by Read tool)
    ///
    /// # Returns
    /// The definition response, which may contain locations where the symbol is defined.
    /// Returns an error if LSP is not configured or the symbol is not found on that line.
    fn goto_definition(
        &self,
        _file_path: &str,
        _symbol: &str,
        _line: u32,
    ) -> impl Future<Output = Result<GotoDefinitionResponse, CodingError>> + Send {
        async { Err(CodingError::NotConfigured("LSP not configured".to_string())) }
    }

    /// Find all references to a symbol.
    ///
    /// # Arguments
    /// * `file_path` - The path to the file containing the symbol
    /// * `symbol` - The symbol name to look up (e.g., "LspClient", "spawn", "HashMap")
    /// * `line` - Line number where the symbol appears (1-indexed, as shown by Read tool)
    /// * `include_declaration` - Whether to include the declaration in the results
    ///
    /// # Returns
    /// A list of locations where the symbol is referenced.
    /// Returns an error if LSP is not configured or the symbol is not found on that line.
    fn find_references(
        &self,
        _file_path: &str,
        _symbol: &str,
        _line: u32,
        _include_declaration: bool,
    ) -> impl Future<Output = Result<Vec<Location>, CodingError>> + Send {
        async { Err(CodingError::NotConfigured("LSP not configured".to_string())) }
    }

    /// Get hover information (type, documentation) for a symbol.
    ///
    /// # Arguments
    /// * `file_path` - The path to the file containing the symbol
    /// * `symbol` - The symbol name to look up (e.g., "LspClient", "spawn", "HashMap")
    /// * `line` - Line number where the symbol appears (1-indexed, as shown by Read tool)
    ///
    /// # Returns
    /// Hover information if available, or None if no information at the position.
    /// Returns an error if LSP is not configured or the symbol is not found on that line.
    fn hover(
        &self,
        _file_path: &str,
        _symbol: &str,
        _line: u32,
    ) -> impl Future<Output = Result<Option<Hover>, CodingError>> + Send {
        async { Err(CodingError::NotConfigured("LSP not configured".to_string())) }
    }

    /// Search for symbols across the workspace.
    ///
    /// # Arguments
    /// * `query` - The search query (fuzzy matching is used by most language servers)
    ///
    /// # Returns
    /// A list of symbols matching the query, including their names, kinds, locations,
    /// and container names. Returns an empty vector if no matches are found.
    /// Returns an error if LSP is not configured.
    fn workspace_symbol(
        &self,
        _query: &str,
    ) -> impl Future<Output = Result<Vec<SymbolInformation>, CodingError>> + Send {
        async { Err(CodingError::NotConfigured("LSP not configured".to_string())) }
    }
}
