//! Agent specification types for authored agent definitions.
//!
//! `AgentSpec` is the canonical abstraction for authored agent definitions across the stack.
//! It represents a resolved runtime type, not a raw settings DTO.

use crate::core::Prompt;
use llm::{LlmModel, ReasoningEffort};
use std::path::{Path, PathBuf};

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
    /// For authored `AgentSpec`s resolved from settings, this contains only authored-safe
    /// prompt variants (e.g., `Prompt::PromptGlobs`). Runtime-owned prompts like
    /// `Prompt::SystemEnv` and `Prompt::McpInstructions` are added separately during
    /// agent construction.
    pub prompts: Vec<Prompt>,
    /// Resolved MCP config path for this agent.
    ///
    /// Before catalog resolution, this holds the agent-local override.
    /// After catalog resolution, this holds the effective path (agent-local > inherited > cwd).
    pub mcp_config_path: Option<PathBuf>,
    /// How this agent can be invoked.
    pub exposure: AgentSpecExposure,
}

impl AgentSpec {
    /// Create a default (no-mode) agent spec with inherited prompts.
    pub fn default_spec(
        model: &LlmModel,
        reasoning_effort: Option<ReasoningEffort>,
        prompts: Vec<Prompt>,
    ) -> Self {
        Self {
            name: "__default__".to_string(),
            description: "Default agent".to_string(),
            model: model.to_string(),
            reasoning_effort,
            prompts,
            mcp_config_path: None,
            exposure: AgentSpecExposure::none(),
        }
    }

    /// Resolve effective MCP config path in place using precedence:
    /// 1. Agent's own `mcp_config_path` (kept as-is)
    /// 2. `inherited_mcp_config_path` (from settings)
    /// 3. `cwd/mcp.json`
    pub fn resolve_mcp_config(&mut self, inherited: Option<&Path>, cwd: &Path) {
        if self.mcp_config_path.is_some() {
            return;
        }
        if let Some(path) = inherited {
            self.mcp_config_path = Some(path.to_path_buf());
            return;
        }
        let cwd_mcp = cwd.join("mcp.json");
        if cwd_mcp.is_file() {
            self.mcp_config_path = Some(cwd_mcp);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_spec() -> AgentSpec {
        AgentSpec {
            name: "test".to_string(),
            description: "Test agent".to_string(),
            model: "anthropic:claude-sonnet-4-5".to_string(),
            reasoning_effort: None,
            prompts: vec![],
            mcp_config_path: None,
            exposure: AgentSpecExposure::both(),
        }
    }

    #[test]
    fn default_spec_has_expected_fields() {
        let model: LlmModel = "anthropic:claude-sonnet-4-5".parse().unwrap();
        let prompts = vec![Prompt::from_globs(
            vec!["BASE.md".to_string()],
            PathBuf::from("/tmp"),
        )];
        let spec = AgentSpec::default_spec(&model, None, prompts.clone());

        assert_eq!(spec.name, "__default__");
        assert_eq!(spec.description, "Default agent");
        assert_eq!(spec.model, model.to_string());
        assert!(spec.reasoning_effort.is_none());
        assert_eq!(spec.prompts.len(), 1);
        assert!(spec.mcp_config_path.is_none());
        assert_eq!(spec.exposure, AgentSpecExposure::none());
    }

    #[test]
    fn resolve_mcp_prefers_agent_local_path() {
        let dir = tempfile::tempdir().unwrap();
        let agent_path = dir.path().join("agent-mcp.json");
        let inherited_path = dir.path().join("inherited-mcp.json");
        fs::write(&agent_path, "{}").unwrap();
        fs::write(&inherited_path, "{}").unwrap();

        let mut spec = make_spec();
        spec.mcp_config_path = Some(agent_path.clone());

        spec.resolve_mcp_config(Some(&inherited_path), dir.path());
        assert_eq!(spec.mcp_config_path, Some(agent_path));
    }

    #[test]
    fn resolve_mcp_falls_back_to_inherited() {
        let dir = tempfile::tempdir().unwrap();
        let inherited_path = dir.path().join("inherited-mcp.json");
        fs::write(&inherited_path, "{}").unwrap();
        fs::write(dir.path().join("mcp.json"), "{}").unwrap();

        let mut spec = make_spec();
        spec.resolve_mcp_config(Some(&inherited_path), dir.path());
        assert_eq!(spec.mcp_config_path, Some(inherited_path));
    }

    #[test]
    fn resolve_mcp_falls_back_to_cwd() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("mcp.json"), "{}").unwrap();

        let mut spec = make_spec();
        spec.resolve_mcp_config(None, dir.path());
        assert_eq!(spec.mcp_config_path, Some(dir.path().join("mcp.json")));
    }

    #[test]
    fn resolve_mcp_returns_none_when_nothing_found() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = make_spec();
        spec.resolve_mcp_config(None, dir.path());
        assert!(spec.mcp_config_path.is_none());
    }
}
