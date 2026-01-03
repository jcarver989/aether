pub mod client;
pub mod error;
pub mod language_metadata;
pub mod lsp_config;
pub mod lsp_utils;
pub mod protocol;
pub mod socket_path;

pub use client::{ClientError, ClientResult, LspClient, ensure_daemon_running};
pub use error::{DaemonError, DaemonResult};
pub use language_metadata::{
    LANGUAGE_METADATA, LanguageMetadata, extensions_for_alias, from_extension, from_lsp_id,
    metadata_for,
};
pub use lsp_config::{LspConfig, get_config_for_language};
pub use lsp_utils::symbol_kind_to_string;
pub use socket_path::{ensure_socket_dir, lockfile_path, socket_path};

pub use protocol::*;
