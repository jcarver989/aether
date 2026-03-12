//! Agent specification types for authored agent definitions.
//!
//! `AgentSpec` is the canonical abstraction for authored agent definitions across the stack.
//! It represents a resolved runtime type, not a raw settings DTO.

use crate::core::Prompt;
use llm::ReasoningEffort;
use std::path::PathBuf;

/// A resolved agent specification ready for runtime use.
///
/// This type is produced by validating and resolving authored agent configuration.
/// All validation happens before constructing these runtime types.
#[derive(Debug, Clone)]
pub struct AgentSpec {
    /// The canonical lookup key for this agent.
    pub name: String,
    /// Human-readable description of this agent's purpose.
    pub description: String,
    /// The validated model spec to use for this agent.
    ///
    /// This is stored as a canonical string so authored settings can represent
    /// both single models (`provider:model`) and alloy specs
    /// (`provider1:model1,provider2:model2`).
    pub model: String,
    /// Optional reasoning effort level for models that support it.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// The prompt stack for this agent.
    ///
    /// For authored AgentSpecs resolved from settings, this contains only authored-safe
    /// prompt variants (e.g., `Prompt::PromptGlobs`). Runtime-owned prompts like
    /// `Prompt::SystemEnv` and `Prompt::McpInstructions` are added separately during
    /// agent construction.
    pub prompts: Vec<Prompt>,
    /// Path to the agent-local MCP config file, if any.
    ///
    /// This represents only the agent-local override, not inherited settings-level MCP.
    pub agent_mcp_config_path: Option<PathBuf>,
    /// How this agent can be invoked.
    pub exposure: AgentSpecExposure,
}

/// Defines how an agent can be invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentSpecExposure {
    /// Whether this agent can be invoked by users (e.g., as an ACP mode).
    pub user_invocable: bool,
    /// Whether this agent can be invoked by other agents (e.g., as a sub-agent).
    pub agent_invocable: bool,
}

impl AgentSpecExposure {
    /// Create an exposure that is neither user nor agent invocable.
    ///
    /// Used internally for synthesized default specs (e.g., no-mode sessions).
    /// Not intended for authored agent definitions — all authored agents must
    /// have at least one invocation surface.
    pub fn none() -> Self {
        Self {
            user_invocable: false,
            agent_invocable: false,
        }
    }

    /// Create an exposure that is only user invocable.
    pub fn user_only() -> Self {
        Self {
            user_invocable: true,
            agent_invocable: false,
        }
    }

    /// Create an exposure that is only agent invocable.
    pub fn agent_only() -> Self {
        Self {
            user_invocable: false,
            agent_invocable: true,
        }
    }

    /// Create an exposure that is both user and agent invocable.
    pub fn both() -> Self {
        Self {
            user_invocable: true,
            agent_invocable: true,
        }
    }
}
