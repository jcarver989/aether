//! Error types for settings loading and validation.

use thiserror::Error;

/// Errors that can occur during settings loading and agent resolution.
#[derive(Debug, Error)]
pub enum SettingsError {
    /// The settings file exists but could not be parsed.
    #[error("Failed to parse settings file: {0}")]
    ParseError(String),

    /// An agent entry has an invalid model string.
    #[error("Invalid model '{model}' for agent '{agent}': {error}")]
    InvalidModel {
        agent: String,
        model: String,
        error: String,
    },

    /// An agent entry has an invalid reasoning effort string.
    #[error("Invalid reasoningEffort '{effort}' for agent '{agent}': {error}")]
    InvalidReasoningEffort {
        agent: String,
        effort: String,
        error: String,
    },

    /// An agent entry is missing required fields.
    #[error("Agent '{agent}' is missing required field: {field}")]
    MissingField { agent: String, field: String },

    /// An agent entry has an empty name.
    #[error("Agent at index {index} has an empty name")]
    EmptyAgentName { index: usize },

    /// An agent entry uses a reserved name.
    #[error("Agent name '{name}' is reserved and cannot be used")]
    ReservedAgentName { name: String },

    /// Duplicate agent names.
    #[error("Duplicate agent name: '{name}'")]
    DuplicateAgentName { name: String },

    /// An agent has no invocation surface enabled.
    #[error(
        "Agent '{agent}' must have at least one invocation flag (userInvocable or agentInvocable)"
    )]
    NoInvocationSurface { agent: String },

    /// A prompt glob pattern is syntactically invalid.
    #[error("Invalid glob pattern '{pattern}' for agent '{agent}': {error}")]
    InvalidGlobPattern {
        agent: String,
        pattern: String,
        error: String,
    },

    /// An inherited prompt glob pattern is syntactically invalid.
    #[error("Invalid inherited glob pattern '{pattern}': {error}")]
    InvalidInheritedGlobPattern { pattern: String, error: String },

    /// A prompt entry resolves to zero files.
    #[error("Prompt entry '{pattern}' for agent '{agent}' resolves to no files")]
    ZeroMatchPrompt { agent: String, pattern: String },

    /// An inherited prompt entry resolves to zero files.
    #[error("Inherited prompt entry '{pattern}' resolves to no files")]
    ZeroMatchInheritedPrompt { pattern: String },

    /// An agent has no prompts after inheritance.
    #[error("Agent '{agent}' has no prompts after inheritance (neither inherited nor local)")]
    NoPrompts { agent: String },

    /// An MCP config path does not exist or is invalid.
    #[error("MCP config path '{path}' does not exist or is not a file")]
    InvalidMcpConfigPath { path: String },

    /// I/O error while reading files.
    #[error("I/O error: {0}")]
    IoError(String),

    /// An agent was not found in the catalog.
    #[error("Agent '{name}' not found")]
    AgentNotFound { name: String },

    /// Duplicate prompt names in the catalog.
    #[error("Duplicate prompt name: '{name}'")]
    DuplicatePromptName { name: String },
}
