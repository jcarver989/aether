#[cfg(feature = "testing")]
pub mod testing;

mod client_handler;
mod file_watcher;
mod lsp_manager;
mod pid_lockfile;

pub mod client;
pub mod daemon;
pub mod error;
pub mod language_id;
pub mod language_metadata;
pub mod lsp_config;
pub mod lsp_utils;
pub mod protocol;
pub mod socket_path;
pub mod uri;

pub use client::{ClientError, ClientResult, LspClient, ensure_daemon_running};
pub use daemon::{LspDaemon, run_daemon};
pub use error::{DaemonError, DaemonResult};
pub use language_id::LanguageId;
pub use language_metadata::{
    LANGUAGE_METADATA, LanguageMetadata, extensions_for_alias, from_lsp_id, metadata_for,
};
pub use lsp_config::{LspConfig, get_config_for_language};
pub use lsp_utils::symbol_kind_to_string;
pub use socket_path::{ensure_socket_dir, lockfile_path, log_file_path, socket_path};

pub use protocol::{
    DaemonRequest, DaemonResponse, InitializeRequest, LspErrorResponse, LspNotification,
    MAX_MESSAGE_SIZE, ProtocolError,
};
pub use uri::{path_to_uri, uri_to_path};
