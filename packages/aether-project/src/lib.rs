//! Project-local `.aether/` configuration handling.
//!
//! This crate owns project-local `.aether/` semantics:
//! - `.aether/settings.json` DTOs
//! - Parsing and validation
//! - Path normalization relative to project root
//! - Resolved catalog/runtime input types
//! - MCP precedence resolution

mod catalog;
mod error;
mod settings;

pub use catalog::{AgentCatalog, ResolvedRuntimeSpec};
pub use error::SettingsError;
pub use settings::load_agent_catalog;
