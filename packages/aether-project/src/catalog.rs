//! Resolved agent catalog and runtime input types.

use crate::error::SettingsError;
use aether_core::agent_spec::{AgentSpec, AgentSpecExposure};
use aether_core::core::Prompt;
use llm::{LlmModel, ReasoningEffort};
use std::path::{Path, PathBuf};

/// A resolved catalog of agents from a project.
///
/// This type owns project-relative/inherited resolution context.
#[derive(Debug, Clone)]
pub struct AgentCatalog {
    /// The project root directory.
    pub(crate) project_root: PathBuf,
    /// Inherited top-level prompts applied to all agents.
    pub(crate) inherited_prompts: Vec<Prompt>,
    /// Path to inherited MCP config from settings, if any.
    pub(crate) inherited_mcp_config_path: Option<PathBuf>,
    /// All resolved agent specs in authored order.
    pub(crate) specs: Vec<AgentSpec>,
}

impl AgentCatalog {
    /// Create an empty catalog for a project with no settings.
    pub fn empty(project_root: PathBuf) -> Self {
        Self {
            project_root,
            inherited_prompts: Vec::new(),
            inherited_mcp_config_path: None,
            specs: Vec::new(),
        }
    }

    /// The project root directory.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Inherited top-level prompts applied to all agents.
    pub fn inherited_prompts(&self) -> &[Prompt] {
        &self.inherited_prompts
    }

    /// Path to inherited MCP config from settings, if any.
    pub fn inherited_mcp_config_path(&self) -> Option<&Path> {
        self.inherited_mcp_config_path.as_deref()
    }

    /// Get all agent specs in the catalog.
    pub fn all(&self) -> &[AgentSpec] {
        &self.specs
    }

    /// Get a specific agent by name.
    pub fn get(&self, name: &str) -> Result<&AgentSpec, SettingsError> {
        self.specs
            .iter()
            .find(|spec| spec.name == name)
            .ok_or_else(|| SettingsError::AgentNotFound {
                name: name.to_string(),
            })
    }

    /// Iterate over user-invocable agents.
    pub fn user_invocable(&self) -> impl Iterator<Item = &AgentSpec> {
        self.specs.iter().filter(|s| s.exposure.user_invocable)
    }

    /// Iterate over agent-invocable agents.
    pub fn agent_invocable(&self) -> impl Iterator<Item = &AgentSpec> {
        self.specs.iter().filter(|s| s.exposure.agent_invocable)
    }

    /// Compute runtime inputs for a named agent.
    ///
    /// This resolves the effective MCP config path using precedence rules:
    /// 1. Agent's `agent_mcp_config_path`
    /// 2. Inherited `inherited_mcp_config_path`
    /// 3. Project `cwd/mcp.json`
    /// 4. None
    ///
    /// Agent lookup is exact by name. Missing agents return an error.
    pub fn runtime_inputs_for(
        &self,
        name: &str,
        cwd: &Path,
    ) -> Result<ResolvedRuntimeSpec, SettingsError> {
        let spec = self.get(name)?;
        let effective_mcp_config_path = self.resolve_effective_mcp_config(spec, cwd);

        Ok(ResolvedRuntimeSpec {
            spec: spec.clone(),
            effective_mcp_config_path,
        })
    }

    /// Compute runtime inputs for a default (no-mode) session.
    ///
    /// This synthesizes a runtime-only default `AgentSpec` with:
    /// - The provided model and reasoning effort
    /// - Inherited top-level prompts
    /// - No agent-local MCP config
    pub fn runtime_inputs_for_default(
        &self,
        model: &LlmModel,
        reasoning_effort: Option<ReasoningEffort>,
        cwd: &Path,
    ) -> ResolvedRuntimeSpec {
        let spec = AgentSpec {
            name: "__default__".to_string(),
            description: "Default agent".to_string(),
            model: model.to_string(),
            reasoning_effort,
            prompts: self.inherited_prompts.clone(),
            agent_mcp_config_path: None,
            exposure: AgentSpecExposure::none(),
        };

        let effective_mcp_config_path = self.resolve_effective_mcp_config(&spec, cwd);

        ResolvedRuntimeSpec {
            spec,
            effective_mcp_config_path,
        }
    }

    /// Resolve the effective MCP config path using precedence rules.
    fn resolve_effective_mcp_config(&self, spec: &AgentSpec, cwd: &Path) -> Option<PathBuf> {
        if let Some(ref path) = spec.agent_mcp_config_path {
            return Some(path.clone());
        }

        if let Some(ref path) = self.inherited_mcp_config_path {
            return Some(path.clone());
        }

        let cwd_mcp = cwd.join("mcp.json");
        if cwd_mcp.is_file() {
            return Some(cwd_mcp);
        }

        None
    }
}

/// Execution-ready inputs for an agent.
#[derive(Debug, Clone)]
pub struct ResolvedRuntimeSpec {
    /// The agent spec.
    pub spec: AgentSpec,
    /// The effective MCP config path after precedence resolution.
    pub effective_mcp_config_path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_temp_project() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn write_file(dir: &Path, path: &str, content: &str) {
        let full_path = dir.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    fn create_test_catalog(project_root: PathBuf) -> AgentCatalog {
        AgentCatalog {
            project_root: project_root.clone(),
            inherited_prompts: vec![Prompt::from_globs(
                vec!["BASE.md".to_string()],
                project_root.clone(),
            )],
            inherited_mcp_config_path: None,
            specs: vec![AgentSpec {
                name: "planner".to_string(),
                description: "Planner agent".to_string(),
                model: "anthropic:claude-sonnet-4-5".to_string(),
                reasoning_effort: None,
                prompts: vec![
                    Prompt::from_globs(vec!["BASE.md".to_string()], project_root.clone()),
                    Prompt::from_globs(vec!["AGENTS.md".to_string()], project_root),
                ],
                agent_mcp_config_path: None,
                exposure: AgentSpecExposure::both(),
            }],
        }
    }

    #[test]
    fn user_invocable_filters_correctly() {
        let dir = create_temp_project();
        let mut catalog = create_test_catalog(dir.path().to_path_buf());
        catalog.specs.push(AgentSpec {
            name: "internal".to_string(),
            description: "Internal agent".to_string(),
            model: "anthropic:claude-sonnet-4-5".to_string(),
            reasoning_effort: None,
            prompts: vec![],
            agent_mcp_config_path: None,
            exposure: AgentSpecExposure::agent_only(),
        });

        let user_invocable: Vec<_> = catalog.user_invocable().collect();
        assert_eq!(user_invocable.len(), 1);
        assert_eq!(user_invocable[0].name, "planner");
    }

    #[test]
    fn agent_invocable_filters_correctly() {
        let dir = create_temp_project();
        let mut catalog = create_test_catalog(dir.path().to_path_buf());
        catalog.specs.push(AgentSpec {
            name: "user-only".to_string(),
            description: "User only agent".to_string(),
            model: "anthropic:claude-sonnet-4-5".to_string(),
            reasoning_effort: None,
            prompts: vec![],
            agent_mcp_config_path: None,
            exposure: AgentSpecExposure::user_only(),
        });

        let agent_invocable: Vec<_> = catalog.agent_invocable().collect();
        assert_eq!(agent_invocable.len(), 1);
        assert_eq!(agent_invocable[0].name, "planner");
    }

    #[test]
    fn get_returns_error_for_missing_agent() {
        let dir = create_temp_project();
        let catalog = create_test_catalog(dir.path().to_path_buf());
        let result = catalog.get("nonexistent");
        assert!(matches!(result, Err(SettingsError::AgentNotFound { .. })));
    }

    #[test]
    fn runtime_inputs_for_missing_agent_returns_error() {
        let dir = create_temp_project();
        let catalog = create_test_catalog(dir.path().to_path_buf());
        let result = catalog.runtime_inputs_for("missing", dir.path());
        assert!(matches!(result, Err(SettingsError::AgentNotFound { .. })));
    }

    #[test]
    fn runtime_inputs_selects_agent_mcp_over_inherited() {
        let dir = create_temp_project();
        write_file(dir.path(), "agent-mcp.json", "{}");
        write_file(dir.path(), "inherited-mcp.json", "{}");

        let mut catalog = create_test_catalog(dir.path().to_path_buf());
        catalog.inherited_mcp_config_path = Some(dir.path().join("inherited-mcp.json"));
        catalog.specs[0].agent_mcp_config_path = Some(dir.path().join("agent-mcp.json"));

        let inputs = catalog.runtime_inputs_for("planner", dir.path()).unwrap();
        assert_eq!(
            inputs.effective_mcp_config_path,
            Some(dir.path().join("agent-mcp.json"))
        );
    }

    #[test]
    fn runtime_inputs_selects_inherited_mcp_over_cwd() {
        let dir = create_temp_project();
        write_file(dir.path(), "inherited-mcp.json", "{}");
        write_file(dir.path(), "mcp.json", "{}");

        let mut catalog = create_test_catalog(dir.path().to_path_buf());
        catalog.inherited_mcp_config_path = Some(dir.path().join("inherited-mcp.json"));

        let inputs = catalog.runtime_inputs_for("planner", dir.path()).unwrap();
        assert_eq!(
            inputs.effective_mcp_config_path,
            Some(dir.path().join("inherited-mcp.json"))
        );
    }

    #[test]
    fn runtime_inputs_falls_back_to_cwd_mcp() {
        let dir = create_temp_project();
        write_file(dir.path(), "mcp.json", "{}");

        let catalog = create_test_catalog(dir.path().to_path_buf());
        let inputs = catalog.runtime_inputs_for("planner", dir.path()).unwrap();
        assert_eq!(
            inputs.effective_mcp_config_path,
            Some(dir.path().join("mcp.json"))
        );
    }

    #[test]
    fn runtime_inputs_no_mcp_config_is_valid() {
        let dir = create_temp_project();
        let catalog = create_test_catalog(dir.path().to_path_buf());
        let inputs = catalog.runtime_inputs_for("planner", dir.path()).unwrap();
        assert!(inputs.effective_mcp_config_path.is_none());
    }

    #[test]
    fn runtime_inputs_for_default_includes_inherited_prompts() {
        let dir = create_temp_project();
        let catalog = create_test_catalog(dir.path().to_path_buf());

        let model: LlmModel = "anthropic:claude-sonnet-4-5".parse().unwrap();
        let inputs = catalog.runtime_inputs_for_default(&model, None, dir.path());

        assert_eq!(inputs.spec.name, "__default__");
        assert_eq!(inputs.spec.model, model.to_string());
        assert_eq!(inputs.spec.prompts.len(), 1);
        assert!(inputs.spec.agent_mcp_config_path.is_none());
    }
}
