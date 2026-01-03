pub mod client;
pub mod error;
pub mod socket_path;

pub use client::LspDaemonClient;
pub use error::DaemonClientError;

pub use aether_lspd::{
    DaemonRequest, DaemonResponse, InitializeRequest, LanguageId, LspErrorResponse,
    LspNotification, LspRequest, LspResponse, ProtocolError, ServerNotification, read_frame,
    write_frame,
};
