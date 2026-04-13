#![doc = include_str!("../README.md")]

mod catalog;
mod error;
mod prompt_catalog;
pub mod prompt_file;
mod settings;

pub use catalog::AgentCatalog;
pub use error::SettingsError;
pub use prompt_catalog::PromptCatalog;
pub use prompt_file::{PromptFile, PromptFileError, PromptTriggers, SKILL_FILENAME};
pub use settings::{AgentEntry, McpServerEntry, Settings, load_agent_catalog};
