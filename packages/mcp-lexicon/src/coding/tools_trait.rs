use std::future::Future;
use std::{collections::HashMap, fmt::Debug};

use super::{
    BackgroundProcessHandle, BashInput, BashResult, EditFileArgs, EditFileResponse, ListFilesArgs,
    ListFilesResult, ReadBackgroundBashOutput, ReadFileArgs, ReadFileResult, WriteFileArgs,
    WriteFileResponse,
};
use lsp_types::{Diagnostic, GotoDefinitionResponse, Hover, Location, Uri};

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
    ) -> impl Future<Output = Result<ReadFileResult, String>> + Send;

    /// Write content to a file
    fn write_file(
        &self,
        args: WriteFileArgs,
    ) -> impl Future<Output = Result<WriteFileResponse, String>> + Send;

    /// Edit a file using string replacement
    fn edit_file(
        &self,
        args: EditFileArgs,
    ) -> impl Future<Output = Result<EditFileResponse, String>> + Send;

    /// List files in a directory
    fn list_files(
        &self,
        args: ListFilesArgs,
    ) -> impl Future<Output = Result<ListFilesResult, String>> + Send;

    /// Execute a bash command
    fn bash(&self, args: BashInput) -> impl Future<Output = Result<BashResult, String>> + Send;

    /// Read output from a background bash process
    fn read_background_bash(
        &self,
        handle: BackgroundProcessHandle,
        filter: Option<String>,
    ) -> impl Future<
        Output = Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), String>,
    > + Send;

    /// Get all cached LSP diagnostics (errors, warnings, etc.).
    ///
    /// Returns diagnostics keyed by file URI.
    /// Returns an error if LSP is not configured for this instance.
    fn get_lsp_diagnostics(
        &self,
    ) -> impl Future<Output = Result<HashMap<Uri, Vec<Diagnostic>>, String>> + Send {
        async { Ok(HashMap::new()) }
    }

    /// Go to the definition of a symbol at a position.
    ///
    /// # Arguments
    /// * `file_path` - The path to the file
    /// * `line` - Line number (1-indexed, as shown in editors)
    /// * `column` - Column number (1-indexed, as shown in editors)
    ///
    /// # Returns
    /// The definition response, which may contain locations where the symbol is defined.
    /// Returns an error if LSP is not configured for this instance.
    fn goto_definition(
        &self,
        _file_path: &str,
        _line: u32,
        _column: u32,
    ) -> impl Future<Output = Result<GotoDefinitionResponse, String>> + Send {
        async { Err("LSP not configured".to_string()) }
    }

    /// Find all references to a symbol at a position.
    ///
    /// # Arguments
    /// * `file_path` - The path to the file
    /// * `line` - Line number (1-indexed, as shown in editors)
    /// * `column` - Column number (1-indexed, as shown in editors)
    /// * `include_declaration` - Whether to include the declaration in the results
    ///
    /// # Returns
    /// A list of locations where the symbol is referenced.
    /// Returns an error if LSP is not configured for this instance.
    fn find_references(
        &self,
        _file_path: &str,
        _line: u32,
        _column: u32,
        _include_declaration: bool,
    ) -> impl Future<Output = Result<Vec<Location>, String>> + Send {
        async { Err("LSP not configured".to_string()) }
    }

    /// Get hover information (type, documentation) for a symbol at a position.
    ///
    /// # Arguments
    /// * `file_path` - The path to the file
    /// * `line` - Line number (1-indexed, as shown in editors)
    /// * `column` - Column number (1-indexed, as shown in editors)
    ///
    /// # Returns
    /// Hover information if available, or None if no information at the position.
    /// Returns an error if LSP is not configured for this instance.
    fn hover(
        &self,
        _file_path: &str,
        _line: u32,
        _column: u32,
    ) -> impl Future<Output = Result<Option<Hover>, String>> + Send {
        async { Err("LSP not configured".to_string()) }
    }
}
