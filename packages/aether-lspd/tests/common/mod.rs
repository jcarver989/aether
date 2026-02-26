pub mod cargo_project;
pub mod daemon_harness;

pub use cargo_project::{CargoProject, TestProject};
pub use daemon_harness::DaemonHarness;

use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, TextDocumentContentChangeEvent,
    TextDocumentItem, VersionedTextDocumentIdentifier,
};

/// Default timeout for rust-analyzer initialization
pub const RA_INIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Helper to create DidOpenTextDocumentParams
pub fn did_open_params(uri: lsp_types::Uri, content: &str) -> DidOpenTextDocumentParams {
    DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri,
            language_id: "rust".to_string(),
            version: 1,
            text: content.to_string(),
        },
    }
}

/// Helper to create DidChangeTextDocumentParams
pub fn did_change_params(
    uri: lsp_types::Uri,
    version: i32,
    new_content: &str,
) -> DidChangeTextDocumentParams {
    DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier { uri, version },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: new_content.to_string(),
        }],
    }
}
