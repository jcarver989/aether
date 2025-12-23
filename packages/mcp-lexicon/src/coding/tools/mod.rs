//! Coding tools module
//!
//! This module contains all the coding tools organized by functionality.

pub mod bash;
pub mod edit_file;
pub mod find;
pub mod grep;
pub mod list_files;
pub mod lsp;
pub mod read_file;
pub mod todo_write;
pub mod write_file;

// Re-export all types for convenience
pub use bash::{
    BackgroundProcessHandle, BashInput, BashOutput, BashResult, ReadBackgroundBashInput,
    ReadBackgroundBashOutput, execute_command, read_background_bash,
};
pub use edit_file::{EditFileArgs, EditFileResponse, edit_file_contents};
pub use find::{FindInput, FindOutput, find_files_by_name};
pub use grep::{GrepInput, GrepOutput, perform_grep};
pub use list_files::{ListFilesArgs, ListFilesResult, list_files};
pub use lsp::LspCodingTools;
pub use lsp::tool::{
    DiagnosticsSummary, LocationResult, LspDiagnostic, LspDiagnosticsInput, LspDiagnosticsOutput,
    LspFindReferencesInput, LspFindReferencesOutput, LspGotoDefinitionInput,
    LspGotoDefinitionOutput, LspHoverInput, LspHoverOutput, LspWorkspaceSymbolInput,
    LspWorkspaceSymbolOutput, SymbolResult, execute_lsp_diagnostics, execute_lsp_find_references,
    execute_lsp_goto_definition, execute_lsp_hover, execute_lsp_workspace_symbol,
};
pub use read_file::{ReadFileArgs, ReadFileResult, read_file_contents};
pub use todo_write::{TodoItem, TodoStatus, TodoWriteInput, TodoWriteOutput, process_todo_write};
pub use write_file::{WriteFileArgs, WriteFileResponse, write_file_contents};
