pub mod common;
pub mod diagnostics;
pub mod error;
pub mod registry;
mod server;
pub mod tools;

pub use registry::LspRegistry;
pub use server::{LspMcp, LspMcpArgs};
