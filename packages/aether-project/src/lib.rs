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
mod prompt_catalog;
pub mod prompt_file;
mod settings;

pub use catalog::{AgentCatalog, ResolvedRuntimeSpec};
pub use error::SettingsError;
pub use prompt_catalog::PromptCatalog;
pub use prompt_file::{PromptFile, PromptFileError, PromptTriggers, SKILL_FILENAME};
pub use settings::load_agent_catalog;
