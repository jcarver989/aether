#[cfg(feature = "mcp-oauth")]
pub mod credential_store;
#[cfg(feature = "mcp-oauth")]
pub mod error;
#[cfg(feature = "mcp-oauth")]
pub mod handler;

#[cfg(feature = "mcp-oauth")]
pub use credential_store::FileCredentialStore;
#[cfg(feature = "mcp-oauth")]
pub use error::OAuthError;
#[cfg(feature = "mcp-oauth")]
pub use handler::{ChannelOAuthHandler, ChannelOAuthHandlerSender, OAuthHandler};
