//! Resolved agent catalog and runtime input types.

use crate::error::SettingsError;
use aether_core::agent_spec::{AgentSpec, McpJsonFileRef};
use aether_core::core::Prompt;
use llm::{LlmModel, ReasoningEffort};
use std::path::{Path, PathBuf};

/// A resolved catalog of agents from a project.
///
/// This type owns project-relative/inherited resolution context.
#[derive(Debug, Clone)]
pub struct AgentCatalog {
    project_root: PathBuf,
    inherited_prompts: Vec<Prompt>,
    inherited_mcp_config_refs: Vec<McpJsonFileRef>,
    specs: Vec<AgentSpec>,
}

impl AgentCatalog {
    /// Create a catalog with the given resolved state.
    pub(crate) fn new(
        project_root: PathBuf,
        inherited_prompts: Vec<Prompt>,
        inherited_mcp_config_refs: Vec<McpJsonFileRef>,
        specs: Vec<AgentSpec>,
    ) -> Self {
        Self { project_root, inherited_prompts, inherited_mcp_config_refs, specs }
    }

    /// Create an empty catalog for a project with no settings.
    pub fn empty(project_root: PathBuf) -> Self {
        Self::new(project_root, Vec::new(), Vec::new(), Vec::new())
    }

    /// The project root directory.
    pub fn project_root(&self) -> &Path {
        &self.project_root
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
            .ok_or_else(|| SettingsError::AgentNotFound { name: name.to_string() })
    }

    /// Iterate over user-invocable agents.
    pub fn user_invocable(&self) -> impl Iterator<Item = &AgentSpec> {
        self.specs.iter().filter(|s| s.exposure.user_invocable)
    }

    /// Iterate over agent-invocable agents.
    pub fn agent_invocable(&self) -> impl Iterator<Item = &AgentSpec> {
        self.specs.iter().filter(|s| s.exposure.agent_invocable)
    }

    /// Resolve and return a named agent spec ready for runtime use.
    pub fn resolve(&self, name: &str, cwd: &Path) -> Result<AgentSpec, SettingsError> {
        let mut spec = self.get(name)?.clone();
        spec.resolve_mcp_config(&self.inherited_mcp_config_refs, cwd);
        Ok(spec)
    }

    /// Resolve and return a default (no-mode) agent spec ready for runtime use.
    pub fn resolve_default(
        &self,
        model: &LlmModel,
        reasoning_effort: Option<ReasoningEffort>,
        cwd: &Path,
    ) -> AgentSpec {
        let mut spec = AgentSpec::default_spec(model, reasoning_effort, self.inherited_prompts.clone());
        spec.resolve_mcp_config(&self.inherited_mcp_config_refs, cwd);
        spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::agent_spec::{AgentSpecExposure, McpJsonFileRef, ToolFilter};
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

    fn make_spec(name: &str, exposure: AgentSpecExposure) -> AgentSpec {
        AgentSpec {
            name: name.to_string(),
            description: format!("{name} agent"),
            model: "anthropic:claude-sonnet-4-5".to_string(),
            reasoning_effort: None,
            prompts: vec![],
            mcp_config_refs: Vec::new(),
            exposure,
            tools: ToolFilter::default(),
        }
    }

    fn create_test_catalog(project_root: PathBuf) -> AgentCatalog {
        let inherited_prompts = vec![Prompt::from_globs(vec!["BASE.md".to_string()], project_root.clone())];
        let planner = AgentSpec {
            name: "planner".to_string(),
            description: "Planner agent".to_string(),
            model: "anthropic:claude-sonnet-4-5".to_string(),
            reasoning_effort: None,
            prompts: vec![
                Prompt::from_globs(vec!["BASE.md".to_string()], project_root.clone()),
                Prompt::from_globs(vec!["AGENTS.md".to_string()], project_root.clone()),
            ],
            mcp_config_refs: Vec::new(),
            exposure: AgentSpecExposure::both(),
            tools: ToolFilter::default(),
        };
        AgentCatalog::new(project_root, inherited_prompts, Vec::new(), vec![planner])
    }

    #[test]
    fn user_invocable_filters_correctly() {
        let dir = create_temp_project();
        let root = dir.path().to_path_buf();
        let catalog = AgentCatalog::new(
            root,
            vec![],
            Vec::new(),
            vec![
                make_spec("planner", AgentSpecExposure::both()),
                make_spec("internal", AgentSpecExposure::agent_only()),
            ],
        );

        let user_invocable: Vec<_> = catalog.user_invocable().collect();
        assert_eq!(user_invocable.len(), 1);
        assert_eq!(user_invocable[0].name, "planner");
    }

    #[test]
    fn agent_invocable_filters_correctly() {
        let dir = create_temp_project();
        let root = dir.path().to_path_buf();
        let catalog = AgentCatalog::new(
            root,
            vec![],
            Vec::new(),
            vec![
                make_spec("planner", AgentSpecExposure::both()),
                make_spec("user-only", AgentSpecExposure::user_only()),
            ],
        );

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
    fn resolve_missing_agent_returns_error() {
        let dir = create_temp_project();
        let catalog = create_test_catalog(dir.path().to_path_buf());
        let result = catalog.resolve("missing", dir.path());
        assert!(matches!(result, Err(SettingsError::AgentNotFound { .. })));
    }

    #[test]
    fn resolve_selects_agent_mcp_over_inherited() {
        let dir = create_temp_project();
        write_file(dir.path(), "agent-mcp.json", "{}");
        write_file(dir.path(), "inherited-mcp.json", "{}");

        let mut planner = make_spec("planner", AgentSpecExposure::both());
        planner.mcp_config_refs = vec![McpJsonFileRef::direct(dir.path().join("agent-mcp.json"))];

        let catalog = AgentCatalog::new(
            dir.path().to_path_buf(),
            vec![],
            vec![McpJsonFileRef::direct(dir.path().join("inherited-mcp.json"))],
            vec![planner],
        );

        let spec = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(spec.mcp_config_refs, vec![McpJsonFileRef::direct(dir.path().join("agent-mcp.json"))]);
    }

    #[test]
    fn resolve_selects_inherited_mcp_over_cwd() {
        let dir = create_temp_project();
        write_file(dir.path(), "inherited-mcp.json", "{}");
        write_file(dir.path(), "mcp.json", "{}");

        let catalog = AgentCatalog::new(
            dir.path().to_path_buf(),
            vec![],
            vec![McpJsonFileRef::direct(dir.path().join("inherited-mcp.json"))],
            vec![make_spec("planner", AgentSpecExposure::both())],
        );

        let spec = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(spec.mcp_config_refs, vec![McpJsonFileRef::direct(dir.path().join("inherited-mcp.json"))]);
    }

    #[test]
    fn resolve_falls_back_to_cwd_mcp() {
        let dir = create_temp_project();
        write_file(dir.path(), "mcp.json", "{}");

        let catalog = AgentCatalog::new(
            dir.path().to_path_buf(),
            vec![],
            Vec::new(),
            vec![make_spec("planner", AgentSpecExposure::both())],
        );

        let spec = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(spec.mcp_config_refs, vec![McpJsonFileRef::direct(dir.path().join("mcp.json"))]);
    }

    #[test]
    fn resolve_no_mcp_config_is_valid() {
        let dir = create_temp_project();
        let catalog = AgentCatalog::new(
            dir.path().to_path_buf(),
            vec![],
            Vec::new(),
            vec![make_spec("planner", AgentSpecExposure::both())],
        );

        let spec = catalog.resolve("planner", dir.path()).unwrap();
        assert!(spec.mcp_config_refs.is_empty());
    }

    #[test]
    fn resolve_default_includes_inherited_prompts() {
        let dir = create_temp_project();
        let catalog = create_test_catalog(dir.path().to_path_buf());

        let model: llm::LlmModel = "anthropic:claude-sonnet-4-5".parse().unwrap();
        let spec = catalog.resolve_default(&model, None, dir.path());

        assert_eq!(spec.name, "__default__");
        assert_eq!(spec.model, model.to_string());
        assert_eq!(spec.prompts.len(), 1);
        assert!(spec.mcp_config_refs.is_empty());
    }
}
