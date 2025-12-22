use std::future::Future;
use std::{collections::HashMap, fmt::Debug};

use super::{
    BackgroundProcessHandle, BashInput, BashResult, EditFileArgs, EditFileResponse, ListFilesArgs,
    ListFilesResult, ReadBackgroundBashOutput, ReadFileArgs, ReadFileResult, WriteFileArgs,
    WriteFileResponse,
};
use lsp_types::{Diagnostic, Uri};

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
}
