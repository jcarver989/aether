//! LSP tools module
//!
//! This module contains LSP-powered tools and the `LspCodingTools` wrapper.
//!
//! `LspCodingTools` provides multi-language LSP support by lazily spawning
//! appropriate language servers based on file type.
//!
//! Import types directly from submodules (e.g., `lsp::diagnostics::LspDiagnosticsInput`).

pub mod coding_tools;
pub mod diagnostics;
pub mod find_references;
pub mod goto_definition;
pub mod hover;
pub mod workspace_symbol;

// Re-export only the primary API type
pub use coding_tools::LspCodingTools;
