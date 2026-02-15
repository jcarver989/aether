//! LSP tools module
//!
//! This module contains LSP-powered tools and the `LspCodingTools` wrapper.
//!
//! `LspCodingTools` provides multi-language LSP support by lazily spawning
//! appropriate language servers based on file type.

// Core wrapper
pub mod coding_tools;

// Consolidated LSP tools
pub mod call_hierarchy;
pub mod check_errors;
pub mod document_info;
pub mod symbol_lookup;

// Re-export only the primary API type
pub use coding_tools::LspCodingTools;
