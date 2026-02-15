//! LSP (Language Server Protocol) client module
//!
//! This module provides functionality to communicate with language servers like
//! `rust-analyzer` for code intelligence features such as diagnostics, go-to-definition,
//! hover information, and more.
//!
//! # Architecture
//!
//! The module is organized into:
//! - [`diagnostics`] - Utilities for working with LSP diagnostics
//! - [`error`] - Error types for LSP operations
//! - [`registry`] - Multi-LSP registry for managing multiple language servers
//!
//! For the LSP client, use `aether_lspd::LspClient` which connects to the shared
//! LSP daemon for efficient resource usage across multiple agents. LSP server
//! configurations are managed by the daemon (see `aether_lspd::config`).
//!
//! # Example
//!
//! ```ignore
//! use aether_lspd::{LspClient, LanguageId};
//! use mcp_coding::lsp::LspRegistry;
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let root = Path::new("/path/to/rust/project");
//!
//!     // Connect to daemon, spawning it if needed
//!     let client = LspClient::connect_or_spawn(root, LanguageId::Rust).await?;
//!
//!     // Make LSP requests
//!     let uri = lsp_types::Url::from_file_path(root.join("src/main.rs")).unwrap();
//!     let response = client.goto_definition(uri, 10, 5).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod common;
pub mod diagnostics;
pub mod error;
pub mod registry;

// Re-export from aether_lspd for convenience
pub use aether_lspd::{
    LanguageId, LspClient, LspConfig, get_config_for_language, symbol_kind_to_string,
};

pub use common::{LocationResult, parse_line, path_to_uri, uri_to_path};
pub use diagnostics::{
    DiagnosticCounts, FormattedDiagnostic, Severity, count_by_severity, filter_by_severity,
    format_diagnostics,
};
pub use error::{LspError, Result};
pub use registry::LspRegistry;
