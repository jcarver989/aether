use std::collections::HashMap;

use lsp_types::Diagnostic;

use super::error::CodingError;
use super::{
    BackgroundProcessHandle, BashInput, BashResult, EditFileArgs, EditFileResponse, FindInput,
    FindOutput, GrepInput, GrepOutput, ListFilesArgs, ListFilesResult, ReadBackgroundBashOutput,
    ReadFileArgs, ReadFileResult, WriteFileArgs, WriteFileResponse, edit_file_contents,
    execute_command, find_files_by_name, list_files, perform_grep, read_background_bash,
    read_file_contents, tools_trait::CodingTools, write_file_contents,
};

/// Default implementation that uses local filesystem operations.
///
/// This is the standard behavior for CodingMcp when running outside
/// of an ACP context. For LSP integration, wrap this with `LspAwareCodingTools`.
#[derive(Debug, Default)]
pub struct DefaultCodingTools;

impl DefaultCodingTools {
    /// Create a new DefaultCodingTools instance
    pub fn new() -> Self {
        Self
    }
}

muahahahaha - broken!

impl CodingTools for DefaultCodingTools {
    async fn read_file(&self, args: ReadFileArgs) -> Result<ReadFileResult, CodingError> {
        read_file_contents(args).await.map_err(CodingError::from)
    }

    async fn write_file(&self, args: WriteFileArgs) -> Result<WriteFileResponse, CodingError> {
        write_file_contents(args).await.map_err(CodingError::from)
    }

    async fn edit_file(&self, args: EditFileArgs) -> Result<EditFileResponse, CodingError> {
        edit_file_contents(args).await.map_err(CodingError::from)
    }

    async fn list_files(&self, args: ListFilesArgs) -> Result<ListFilesResult, CodingError> {
        list_files(args).await.map_err(CodingError::from)
    }

    async fn bash(&self, args: BashInput) -> Result<BashResult, CodingError> {
        execute_command(args).await.map_err(CodingError::from)
    }

    async fn read_background_bash(
        &self,
        handle: BackgroundProcessHandle,
        filter: Option<String>,
    ) -> Result<(ReadBackgroundBashOutput, Option<BackgroundProcessHandle>), CodingError> {
        read_background_bash(handle, filter)
            .await
            .map_err(CodingError::from)
    }

    async fn grep(&self, args: GrepInput) -> Result<GrepOutput, CodingError> {
        perform_grep(args).await.map_err(CodingError::from)
    }

    async fn find(&self, args: FindInput) -> Result<FindOutput, CodingError> {
        find_files_by_name(args).await.map_err(CodingError::from)
    }

    async fn get_lsp_diagnostics(&self) -> Result<HashMap<String, Vec<Diagnostic>>, CodingError> {
        // DefaultCodingTools without wrapper has no LSP
        // Wrap with LspAwareCodingTools to enable LSP
        Err(CodingError::NotConfigured("LSP not configured. Wrap DefaultCodingTools with LspAwareCodingTools to enable LSP integration.".to_string()))
    }
}
