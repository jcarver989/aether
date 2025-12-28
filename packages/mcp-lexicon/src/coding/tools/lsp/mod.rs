//! LSP tools module
//!
//! This module contains LSP-powered tools and the `LspCodingTools` wrapper.
//!
//! `LspCodingTools` provides multi-language LSP support by lazily spawning
//! appropriate language servers based on file type.
//!
//! Import types directly from submodules (e.g., `lsp::check_errors::LspDiagnosticsInput`).

pub mod check_errors;
pub mod coding_tools;
pub mod find_definition;
pub mod find_usages;
pub mod get_type_info;
pub mod search_symbols;

// Re-export only the primary API type
pub use coding_tools::LspCodingTools;
