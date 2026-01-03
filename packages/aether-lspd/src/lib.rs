pub mod client;
pub mod config;
pub mod error;
pub mod protocol;
pub mod socket_path;

pub use client::{ClientError, ClientResult, LspClient, ensure_daemon_running};
pub use config::{LspConfig, default_lsp_configs, get_config_for_language};
pub use error::{DaemonError, DaemonResult};
pub use socket_path::{ensure_socket_dir, lockfile_path, socket_path};

pub use protocol::*;
