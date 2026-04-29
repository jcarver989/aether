//! Resolved agent catalog and runtime input types.

use crate::error::SettingsError;
use aether_core::agent_spec::{AgentSpec, McpConfigSource};
use llm::{LlmModel, ReasoningEffort};
use std::path::{Path, PathBuf};

/// A resolved catalog of agents from a project.
///
/// This type owns project-relative resolution context.
#[derive(Debug, Clone)]
pub struct AgentCatalog {
    project_root: PathBuf,
    specs: Vec<AgentSpec>,
    selected_agent: Option<String>,
}

impl AgentCatalog {
    pub(crate) fn new(project_root: PathBuf, specs: Vec<AgentSpec>, selected_agent: Option<String>) -> Self {
        Self { project_root, specs, selected_agent }
    }

    /// Create an empty catalog for a project with no settings.
    pub fn empty(project_root: PathBuf) -> Self {
        Self::new(project_root, Vec::new(), None)
    }

    /// The project root directory.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Get all agent specs in the catalog.
    pub fn all(&self) -> &[AgentSpec] {
        &self.specs
    }

    pub fn selected_agent(&self) -> Option<&str> {
        self.selected_agent.as_deref()
    }

    pub fn default_agent(&self) -> Option<&AgentSpec> {
        self.selected_agent
            .as_deref()
            .and_then(|name| self.specs.iter().find(|spec| spec.name == name))
            .or_else(|| self.user_invocable().next())
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
        self.get(name).cloned().map(|spec| with_default_mcp_config(spec, cwd))
    }

    /// Resolve and return a default (no-mode) agent spec ready for runtime use.
    pub fn resolve_default(
        &self,
        model: &LlmModel,
        reasoning_effort: Option<ReasoningEffort>,
        cwd: &Path,
    ) -> AgentSpec {
        with_default_mcp_config(AgentSpec::default_spec(model, reasoning_effort, Vec::new()), cwd)
    }
}

fn with_default_mcp_config(mut spec: AgentSpec, cwd: &Path) -> AgentSpec {
    if spec.mcp_config_sources.is_empty() {
        let cwd_mcp = cwd.join("mcp.json");
        if cwd_mcp.is_file() {
            spec.mcp_config_sources = vec![McpConfigSource::direct(cwd_mcp)];
        }
    }

    spec
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::agent_spec::{AgentSpecExposure, ToolFilter};
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
            mcp_config_sources: Vec::new(),
            exposure,
            tools: ToolFilter::default(),
        }
    }

    fn create_test_catalog(project_root: PathBuf) -> AgentCatalog {
        let planner = make_spec("planner", AgentSpecExposure::both());
        AgentCatalog::new(project_root, vec![planner], None)
    }

    fn file_sources(spec: &AgentSpec) -> Vec<(PathBuf, bool)> {
        spec.mcp_config_sources
            .iter()
            .filter_map(|source| match source {
                McpConfigSource::File { path, proxy } => Some((path.clone(), *proxy)),
                McpConfigSource::Json(_) | McpConfigSource::Inline(_) => None,
            })
            .collect()
    }

    #[test]
    fn user_invocable_filters_correctly() {
        let dir = create_temp_project();
        let root = dir.path().to_path_buf();
        let catalog = AgentCatalog::new(
            root,
            vec![
                make_spec("planner", AgentSpecExposure::both()),
                make_spec("internal", AgentSpecExposure::agent_only()),
            ],
            None,
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
            vec![
                make_spec("planner", AgentSpecExposure::both()),
                make_spec("user-only", AgentSpecExposure::user_only()),
            ],
            None,
        );

        let agent_invocable: Vec<_> = catalog.agent_invocable().collect();
        assert_eq!(agent_invocable.len(), 1);
        assert_eq!(agent_invocable[0].name, "planner");
    }

    #[test]
    fn default_agent_uses_selected_agent() {
        let dir = create_temp_project();
        let catalog = AgentCatalog::new(
            dir.path().to_path_buf(),
            vec![make_spec("first", AgentSpecExposure::both()), make_spec("second", AgentSpecExposure::both())],
            Some("second".to_string()),
        );

        assert_eq!(catalog.default_agent().map(|spec| spec.name.as_str()), Some("second"));
    }

    #[test]
    fn default_agent_falls_back_to_first_user_invocable() {
        let dir = create_temp_project();
        let catalog = AgentCatalog::new(
            dir.path().to_path_buf(),
            vec![
                make_spec("internal", AgentSpecExposure::agent_only()),
                make_spec("visible", AgentSpecExposure::user_only()),
            ],
            None,
        );

        assert_eq!(catalog.default_agent().map(|spec| spec.name.as_str()), Some("visible"));
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
    fn resolve_selects_agent_mcp_over_cwd() {
        let dir = create_temp_project();
        write_file(dir.path(), "agent-mcp.json", "{}");
        write_file(dir.path(), "mcp.json", "{}");

        let mut planner = make_spec("planner", AgentSpecExposure::both());
        planner.mcp_config_sources = vec![McpConfigSource::direct(dir.path().join("agent-mcp.json"))];

        let catalog = AgentCatalog::new(dir.path().to_path_buf(), vec![planner], None);

        let spec = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(file_sources(&spec), vec![(dir.path().join("agent-mcp.json"), false)]);
    }

    #[test]
    fn resolve_falls_back_to_cwd_mcp() {
        let dir = create_temp_project();
        write_file(dir.path(), "mcp.json", "{}");

        let catalog =
            AgentCatalog::new(dir.path().to_path_buf(), vec![make_spec("planner", AgentSpecExposure::both())], None);

        let spec = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(file_sources(&spec), vec![(dir.path().join("mcp.json"), false)]);
    }

    #[test]
    fn resolve_no_mcp_config_is_valid() {
        let dir = create_temp_project();
        let catalog =
            AgentCatalog::new(dir.path().to_path_buf(), vec![make_spec("planner", AgentSpecExposure::both())], None);

        let spec = catalog.resolve("planner", dir.path()).unwrap();
        assert!(spec.mcp_config_sources.is_empty());
    }

    #[test]
    fn resolve_default_uses_default_spec() {
        let dir = create_temp_project();
        let catalog = create_test_catalog(dir.path().to_path_buf());

        let model: llm::LlmModel = "anthropic:claude-sonnet-4-5".parse().unwrap();
        let spec = catalog.resolve_default(&model, None, dir.path());

        assert_eq!(spec.name, "__default__");
        assert_eq!(spec.model, model.to_string());
        assert!(spec.prompts.is_empty());
        assert!(spec.mcp_config_sources.is_empty());
    }
}
