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
//! use mcp_lexicon::coding::lsp::{LspClient, ClientNotification, ServerNotification};
//! use lsp_types::DidOpenTextDocumentParams;
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let root = Path::new("/path/to/rust/project");
//!
//!     // Spawn and initialize rust-analyzer
//!     let (tx, mut rx, mut client) = LspClient::spawn("rust-analyzer", &[], root).await?;
//!
//!     // Open a file via the notification sender
//!     tx.send(ClientNotification::TextDocumentOpened(DidOpenTextDocumentParams {
//!         text_document: lsp_types::TextDocumentItem {
//!             uri: lsp_types::Url::from_file_path(root.join("src/main.rs")).unwrap(),
//!             language_id: "rust".into(),
//!             version: 1,
//!             text: std::fs::read_to_string(root.join("src/main.rs"))?,
//!         },
//!     })).await?;
//!
//!     // Receive diagnostics from the notification receiver
//!     while let Some(notif) = rx.recv().await {
//!         if let ServerNotification::Diagnostics(diag) = notif {
//!             for d in &diag.diagnostics {
//!                 println!("{:?}: {}", d.severity, d.message);
//!             }
//!         }
//!     }
//!
//!     // Shutdown
//!     client.shutdown().await?;
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod common;
pub mod diagnostics;
pub mod error;
pub mod lsp_diagnostics;
pub mod lsp_find_references;
pub mod lsp_goto_definition;
pub mod lsp_hover;
pub mod lsp_workspace_symbol;
pub mod transport;

pub use client::{
    LspClient, NotificationReceiver, NotificationSender, ServerNotification, path_to_uri,
};
pub use common::{LocationResult, parse_line, symbol_kind_to_string, uri_to_path};
pub use diagnostics::{
    DiagnosticCounts, FormattedDiagnostic, Severity, count_by_severity, filter_by_severity,
    format_diagnostics,
};
pub use error::{LspError, Result};
pub use lsp_diagnostics::{
    DiagnosticsSummary, LspDiagnostic, LspDiagnosticsInput, LspDiagnosticsOutput,
    execute_lsp_diagnostics,
};
pub use lsp_find_references::{
    LspFindReferencesInput, LspFindReferencesOutput, execute_lsp_find_references,
};
pub use lsp_goto_definition::{
    LspGotoDefinitionInput, LspGotoDefinitionOutput, execute_lsp_goto_definition,
};
pub use lsp_hover::{LspHoverInput, LspHoverOutput, execute_lsp_hover};
pub use lsp_workspace_symbol::{
    LspWorkspaceSymbolInput, LspWorkspaceSymbolOutput, SymbolResult, execute_lsp_workspace_symbol,
};
pub use transport::{ClientNotification, LanguageId};
