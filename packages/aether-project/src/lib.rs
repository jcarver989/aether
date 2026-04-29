#![doc = include_str!("../README.md")]

mod catalog;
pub mod config;
mod error;
mod prompt_catalog;
pub mod prompt_file;

pub use catalog::AgentCatalog;
pub use config::{
    AetherConfig, AetherConfigSource, AgentConfig, McpConfigSourceConfig, PromptSource, load_aether_config,
    load_aether_config_from_source, load_agent_catalog, load_agent_catalog_from_source, resolve_config,
};
pub use error::SettingsError;
pub use prompt_catalog::PromptCatalog;
pub use prompt_file::{PromptFile, PromptFileError, PromptTriggers, SKILL_FILENAME};
