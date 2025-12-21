//! LSP (Language Server Protocol) client module
//!
//! This module provides functionality to communicate with language servers like
//! `rust-analyzer` for code intelligence features such as diagnostics, go-to-definition,
//! hover information, and more.
//!
//! # Architecture
//!
//! The module is organized into:
//! - [`client`] - The main `LspClient` struct that manages server lifecycle
//! - [`transport`] - JSON-RPC over stdio transport layer
//! - [`diagnostics`] - Utilities for working with LSP diagnostics
//! - [`error`] - Error types for LSP operations
//!
//! # Example
//!
//! ```ignore
//! use mcp_lexicon::coding::lsp::LspClient;
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Spawn rust-analyzer
//!     let mut client = LspClient::spawn("rust-analyzer", &[])?;
//!
//!     // Initialize with project root
//!     let root = Path::new("/path/to/rust/project");
//!     client.initialize(root).await?;
//!
//!     // Open a file and wait for diagnostics
//!     let uri = lsp_types::Url::from_file_path(root.join("src/main.rs")).unwrap();
//!     let content = std::fs::read_to_string(root.join("src/main.rs"))?;
//!     client.did_open(uri, "rust", content)?;
//!
//!     // Wait for diagnostics
//!     while let Some(diag) = client.recv_diagnostics().await {
//!         for d in &diag.diagnostics {
//!             println!("{}: {}", d.severity.unwrap_or(lsp_types::DiagnosticSeverity::ERROR), d.message);
//!         }
//!     }
//!
//!     // Shutdown
//!     client.shutdown().await?;
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod diagnostics;
pub mod error;
pub mod transport;

pub use client::{DiagnosticsCache, LspClient, LspNotification, path_to_uri};
pub use diagnostics::{
    DiagnosticCounts, FormattedDiagnostic, Severity, count_by_severity, filter_by_severity,
    format_diagnostics,
};
pub use error::{LspError, Result};
