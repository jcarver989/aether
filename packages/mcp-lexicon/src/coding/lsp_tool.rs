//! LSP tools for querying language server information
//!
//! This module re-exports LSP tool types and functions from the `lsp` module.
//! Each tool is implemented in its own submodule for better organization.

pub use super::lsp::{
    // Common types
    LocationResult,
    // Diagnostics tool
    DiagnosticsSummary,
    LspDiagnostic,
    LspDiagnosticsInput,
    LspDiagnosticsOutput,
    execute_lsp_diagnostics,
    // Find references tool
    LspFindReferencesInput,
    LspFindReferencesOutput,
    execute_lsp_find_references,
    // Goto definition tool
    LspGotoDefinitionInput,
    LspGotoDefinitionOutput,
    execute_lsp_goto_definition,
    // Hover tool
    LspHoverInput,
    LspHoverOutput,
    execute_lsp_hover,
    // Workspace symbol tool
    LspWorkspaceSymbolInput,
    LspWorkspaceSymbolOutput,
    SymbolResult,
    execute_lsp_workspace_symbol,
};
