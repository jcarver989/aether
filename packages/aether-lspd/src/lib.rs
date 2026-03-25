#[cfg(feature = "testing")]
pub mod testing;

mod client_connection;
mod diagnostics_store;
mod document_coordinator;
mod file_watcher;
pub mod language_catalog;
mod pid_lockfile;
mod process_transport;
mod workspace_registry;
mod workspace_session;

mod client;
pub mod daemon;
pub mod error;
pub mod lsp_utils;
pub mod protocol;
pub mod socket_path;
pub mod uri;

pub use client::{ClientError, ClientResult, LspClient};
pub use daemon::{LspDaemon, run_daemon};
pub use error::{DaemonError, DaemonResult};
pub use language_catalog::LanguageId;
pub use language_catalog::{
    LANGUAGE_METADATA, LanguageMetadata, LspConfig, extensions_for_alias, from_lsp_id,
    get_config_for_language, metadata_for,
};
pub use lsp_utils::symbol_kind_to_string;
pub use socket_path::{ensure_socket_dir, lockfile_path, log_file_path, socket_path};

pub use protocol::{
    DaemonRequest, DaemonResponse, InitializeRequest, LspErrorResponse, MAX_MESSAGE_SIZE,
    ProtocolError,
};
pub use uri::{path_to_uri, uri_to_path};
