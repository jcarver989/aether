//! Settings file parsing and validation.

use crate::error::SettingsError;
use aether_core::agent_spec::{AgentSpec, AgentSpecExposure, McpJsonFileRef, ToolFilter};
use aether_core::core::Prompt;
use glob::glob;
use llm::{LlmModel, ReasoningEffort};
use std::collections::HashSet;
use std::path::Path;

/// An entry in the `mcpServers` array: either a plain path string or an object with options.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum McpServerEntry {
    Path(String),
    Config {
        path: String,
        #[serde(default)]
        proxy: bool,
    },
}

impl McpServerEntry {
    pub fn path_str(&self) -> &str {
        match self {
            McpServerEntry::Path(p) => p,
            McpServerEntry::Config { path, .. } => path,
        }
    }

    pub fn proxy(&self) -> bool {
        match self {
            McpServerEntry::Path(_) => false,
            McpServerEntry::Config { proxy, .. } => *proxy,
        }
    }
}

impl From<&str> for McpServerEntry {
    fn from(s: &str) -> Self {
        Self::Path(s.to_string())
    }
}

/// Settings DTO for `.aether/settings.json`.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Inherited prompts for all agents.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<String>,
    /// Inherited MCP configs for all agents, applied in order (last wins on collisions).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<McpServerEntry>,
    /// The canonical authored agent registry.
    pub agents: Vec<AgentEntry>,
}

/// Agent entry DTO for `.aether/settings.json`.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentEntry {
    pub name: String,
    pub description: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default)]
    pub user_invocable: bool,
    #[serde(default)]
    pub agent_invocable: bool,
    #[serde(default)]
    pub prompts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<McpServerEntry>,
    #[serde(default, skip_serializing_if = "ToolFilter::is_empty")]
    pub tools: ToolFilter,
}

/// Load and resolve the agent catalog from a project root.
///
/// If `.aether/settings.json` is absent, returns a valid empty catalog.
/// If the settings file is malformed or contains invalid entries, returns an error.
pub fn load_agent_catalog(project_root: &Path) -> Result<super::catalog::AgentCatalog, SettingsError> {
    let settings_path = project_root.join(".aether/settings.json");

    let settings = match std::fs::read_to_string(&settings_path) {
        Ok(content) => {
            if content.trim().is_empty() {
                Settings::default()
            } else {
                serde_json::from_str(&content).map_err(|e| SettingsError::ParseError(e.to_string()))?
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(super::catalog::AgentCatalog::empty(project_root.to_path_buf()));
        }
        Err(e) => {
            return Err(SettingsError::IoError(format!("Failed to read {}: {}", settings_path.display(), e)));
        }
    };

    resolve_settings(project_root, settings)
}

/// Resolve settings into a catalog of agent specs.
fn resolve_settings(project_root: &Path, settings: Settings) -> Result<super::catalog::AgentCatalog, SettingsError> {
    let Settings { prompts: inherited_patterns, mcp_servers, agents } = settings;

    validate_prompt_entries(project_root, &inherited_patterns, None)?;
    let inherited_mcp_config_refs = resolve_mcp_config_refs(project_root, &mcp_servers)?;
    let inherited_prompts = build_inherited_prompts(&inherited_patterns, project_root);

    let mut seen_names = HashSet::new();
    let mut specs = Vec::with_capacity(agents.len());

    for (index, entry) in agents.into_iter().enumerate() {
        specs.push(resolve_agent_entry(project_root, &inherited_prompts, entry, index, &mut seen_names)?);
    }

    Ok(super::catalog::AgentCatalog::new(
        project_root.to_path_buf(),
        inherited_prompts,
        inherited_mcp_config_refs,
        specs,
    ))
}

fn resolve_agent_entry(
    project_root: &Path,
    inherited_prompts: &[Prompt],
    entry: AgentEntry,
    index: usize,
    seen_names: &mut HashSet<String>,
) -> Result<AgentSpec, SettingsError> {
    let name = entry.name.trim().to_string();
    if name.is_empty() {
        return Err(SettingsError::EmptyAgentName { index });
    }

    if name == "__default__" {
        return Err(SettingsError::ReservedAgentName { name });
    }

    if !seen_names.insert(name.clone()) {
        return Err(SettingsError::DuplicateAgentName { name });
    }

    let description = entry.description.trim().to_string();
    if description.is_empty() {
        return Err(SettingsError::MissingField { agent: name.clone(), field: "description".to_string() });
    }

    let model = parse_model(&name, &entry.model)?;

    if !entry.user_invocable && !entry.agent_invocable {
        return Err(SettingsError::NoInvocationSurface { agent: name.clone() });
    }

    validate_prompt_entries(project_root, &entry.prompts, Some(&name))?;

    if inherited_prompts.is_empty() && entry.prompts.is_empty() {
        return Err(SettingsError::NoPrompts { agent: name.clone() });
    }

    let mcp_config_refs = resolve_mcp_config_refs(project_root, &entry.mcp_servers)?;

    let prompts = if entry.prompts.is_empty() {
        inherited_prompts.to_vec()
    } else {
        entry.prompts.iter().map(|p| Prompt::from_globs(vec![p.clone()], project_root.to_path_buf())).collect()
    };

    Ok(AgentSpec {
        name,
        description,
        model,
        reasoning_effort: entry.reasoning_effort,
        prompts,
        mcp_config_refs,
        exposure: AgentSpecExposure { user_invocable: entry.user_invocable, agent_invocable: entry.agent_invocable },
        tools: entry.tools,
    })
}

fn parse_model(agent: &str, model: &str) -> Result<String, SettingsError> {
    canonicalize_model_spec(model).map_err(|error| SettingsError::InvalidModel {
        agent: agent.to_string(),
        model: model.to_string(),
        error,
    })
}

fn canonicalize_model_spec(model: &str) -> Result<String, String> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return Err("Model spec cannot be empty".to_string());
    }

    let mut canonical_parts = Vec::new();
    for part in trimmed.split(',').map(str::trim) {
        if part.is_empty() {
            return Err("Model spec contains an empty entry".to_string());
        }

        part.parse::<LlmModel>().map_err(|error: String| error)?;
        canonical_parts.push(part.to_string());
    }

    Ok(canonical_parts.join(","))
}

fn validate_prompt_entries(
    project_root: &Path,
    patterns: &[String],
    agent_name: Option<&str>,
) -> Result<(), SettingsError> {
    for pattern in patterns {
        validate_prompt_entry(project_root, pattern, agent_name)?;
    }
    Ok(())
}

fn resolve_mcp_config_refs(
    project_root: &Path,
    entries: &[McpServerEntry],
) -> Result<Vec<McpJsonFileRef>, SettingsError> {
    let mut resolved = Vec::with_capacity(entries.len());
    for entry in entries {
        let full_path = project_root.join(entry.path_str());
        if full_path.is_file() {
            resolved.push(McpJsonFileRef { path: full_path, proxy: entry.proxy() });
        } else {
            return Err(SettingsError::InvalidMcpConfigPath { path: entry.path_str().to_string() });
        }
    }
    Ok(resolved)
}

/// Validate that a prompt entry resolves to at least one file.
fn validate_prompt_entry(project_root: &Path, pattern: &str, agent_name: Option<&str>) -> Result<(), SettingsError> {
    let full_pattern = if Path::new(pattern).is_absolute() {
        pattern.to_string()
    } else {
        project_root.join(pattern).to_string_lossy().to_string()
    };

    let has_file_match = glob(&full_pattern)
        .map_err(|e| {
            if let Some(agent) = agent_name {
                SettingsError::InvalidGlobPattern {
                    agent: agent.to_string(),
                    pattern: pattern.to_string(),
                    error: e.to_string(),
                }
            } else {
                SettingsError::InvalidInheritedGlobPattern { pattern: pattern.to_string(), error: e.to_string() }
            }
        })?
        .filter_map(Result::ok)
        .any(|path| path.is_file());

    if has_file_match {
        Ok(())
    } else if let Some(agent) = agent_name {
        Err(SettingsError::ZeroMatchPrompt { agent: agent.to_string(), pattern: pattern.to_string() })
    } else {
        Err(SettingsError::ZeroMatchInheritedPrompt { pattern: pattern.to_string() })
    }
}

/// Build the inherited prompts from patterns.
///
/// Each pattern becomes one `Prompt::PromptGlobs` value.
fn build_inherited_prompts(patterns: &[String], project_root: &Path) -> Vec<Prompt> {
    patterns.iter().map(|pattern| Prompt::from_globs(vec![pattern.clone()], project_root.to_path_buf())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::agent_spec::McpJsonFileRef;
    use std::fs;

    fn create_temp_project() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn write_settings(dir: &Path, content: &str) {
        let aether_dir = dir.join(".aether");
        fs::create_dir_all(&aether_dir).unwrap();
        fs::write(aether_dir.join("settings.json"), content).unwrap();
    }

    fn write_file(dir: &Path, path: &str, content: &str) {
        let full_path = dir.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    /// Standard agent JSON with customizable fields. `extra` is injected into the agent object.
    fn agent_settings(extra: &str) -> String {
        let comma = if extra.is_empty() { "" } else { "," };
        format!(
            r#"{{"agents": [{{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]{comma} {extra}}}]}}"#
        )
    }

    /// Setup a project with AGENTS.md, write settings JSON, and load the catalog.
    fn setup_and_load(json: &str) -> (tempfile::TempDir, Result<super::super::catalog::AgentCatalog, SettingsError>) {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(dir.path(), json);
        let result = load_agent_catalog(dir.path());
        (dir, result)
    }

    fn setup_and_load_ok(json: &str) -> (tempfile::TempDir, super::super::catalog::AgentCatalog) {
        let (dir, result) = setup_and_load(json);
        (dir, result.unwrap())
    }

    #[test]
    fn missing_settings_yields_empty_catalog() {
        let dir = create_temp_project();
        let catalog = load_agent_catalog(dir.path()).unwrap();
        assert!(catalog.all().is_empty());
    }

    #[test]
    fn exposure_flags_parsed_correctly() {
        for (user, agent) in [(true, true), (true, false), (false, true)] {
            let json = format!(
                r#"{{"agents": [{{
                    "name": "planner", "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": {user}, "agentInvocable": {agent},
                    "prompts": ["AGENTS.md"]
                }}]}}"#
            );
            let (_, catalog) = setup_and_load_ok(&json);
            let spec = catalog.get("planner").unwrap();
            assert_eq!(spec.exposure.user_invocable, user);
            assert_eq!(spec.exposure.agent_invocable, agent);
        }
    }

    #[test]
    fn invalid_model_string_rejected() {
        let (_, result) = setup_and_load(
            r#"{"agents": [{"name": "planner", "description": "Planner agent", "model": "invalid:model", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );
        assert!(matches!(result, Err(SettingsError::InvalidModel { .. })));
    }

    #[test]
    fn alloy_model_string_is_accepted() {
        let json = r#"{"agents": [{"name": "alloy", "description": "Alloy agent", "model": "anthropic:claude-sonnet-4-5,deepseek:deepseek-chat", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#;
        let (_, catalog) = setup_and_load_ok(json);
        assert_eq!(catalog.get("alloy").unwrap().model.clone(), "anthropic:claude-sonnet-4-5,deepseek:deepseek-chat");
    }

    #[test]
    fn alloy_model_string_with_unknown_member_is_rejected() {
        let (_, result) = setup_and_load(
            r#"{"agents": [{"name": "alloy", "description": "Alloy agent", "model": "anthropic:claude-sonnet-4-5,mystery:nope", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );
        assert!(matches!(result, Err(SettingsError::InvalidModel { .. })));
    }

    #[test]
    fn invalid_reasoning_effort_rejected() {
        let (_, result) = setup_and_load(&agent_settings(r#""reasoningEffort": "invalid""#));
        assert!(matches!(result, Err(SettingsError::ParseError(_))));
    }

    #[test]
    fn duplicate_agent_names_rejected() {
        let (_, result) = setup_and_load(
            r#"{"agents": [
                {"name": "planner", "description": "First", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]},
                {"name": "planner", "description": "Second", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}
            ]}"#,
        );
        assert!(matches!(result, Err(SettingsError::DuplicateAgentName { .. })));
    }

    #[test]
    fn agent_prompts_override_inherited() {
        let dir = create_temp_project();
        write_file(dir.path(), "BASE.md", "Base instructions");
        write_file(dir.path(), "AGENTS.md", "Agent instructions");
        write_settings(
            dir.path(),
            r#"{"prompts": ["BASE.md"], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "agentInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        // Agent-local prompts override inherited, not additive
        assert_eq!(catalog.get("planner").unwrap().prompts.len(), 1);
    }

    #[test]
    fn agent_without_prompts_inherits_top_level() {
        let dir = create_temp_project();
        write_file(dir.path(), "BASE.md", "Base instructions");
        write_settings(
            dir.path(),
            r#"{"prompts": ["BASE.md"], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        assert_eq!(catalog.get("planner").unwrap().prompts.len(), 1);
    }

    #[test]
    fn one_prompt_globs_per_entry() {
        let dir = create_temp_project();
        write_file(dir.path(), "a.md", "A");
        write_file(dir.path(), "b.md", "B");
        write_settings(
            dir.path(),
            r#"{"agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["a.md", "b.md"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        // Should have 2 PromptGlobs entries, not 1 combined
        assert_eq!(catalog.get("planner").unwrap().prompts.len(), 2);
    }

    #[test]
    fn zero_match_prompt_rejected() {
        let dir = create_temp_project();
        // No AGENTS.md created — prompt won't match
        write_settings(
            dir.path(),
            r#"{"agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["nonexistent.md"]}]}"#,
        );
        assert!(matches!(load_agent_catalog(dir.path()), Err(SettingsError::ZeroMatchPrompt { .. })));
    }

    #[test]
    fn prompt_matching_only_directories_is_rejected() {
        let dir = create_temp_project();
        std::fs::create_dir_all(dir.path().join("prompts")).unwrap();
        write_settings(
            dir.path(),
            r#"{"agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["prompts/*"]}]}"#,
        );
        assert!(matches!(load_agent_catalog(dir.path()), Err(SettingsError::ZeroMatchPrompt { .. })));
    }

    #[test]
    fn no_invocation_surface_rejected() {
        let (_, result) = setup_and_load(
            r#"{"agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": false, "agentInvocable": false, "prompts": ["AGENTS.md"]}]}"#,
        );
        assert!(matches!(result, Err(SettingsError::NoInvocationSurface { .. })));
    }

    #[test]
    fn empty_and_whitespace_names_rejected() {
        for name in ["", "   "] {
            let json = format!(
                r#"{{"agents": [{{"name": "{name}", "description": "Agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}}]}}"#
            );
            let (_, result) = setup_and_load(&json);
            assert!(
                matches!(result, Err(SettingsError::EmptyAgentName { .. })),
                "expected EmptyAgentName for name={name:?}"
            );
        }
    }

    #[test]
    fn empty_and_whitespace_descriptions_rejected() {
        for desc in ["", "   "] {
            let json = format!(
                r#"{{"agents": [{{"name": "planner", "description": "{desc}", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}}]}}"#
            );
            let (_, result) = setup_and_load(&json);
            assert!(
                matches!(result, Err(SettingsError::MissingField { .. })),
                "expected MissingField for desc={desc:?}"
            );
        }
    }

    #[test]
    fn duplicate_agent_names_after_trim_rejected() {
        let (_, result) = setup_and_load(
            r#"{"agents": [
                {"name": "planner", "description": "First", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]},
                {"name": " planner ", "description": "Second", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}
            ]}"#,
        );
        assert!(matches!(result, Err(SettingsError::DuplicateAgentName { .. })));
    }

    #[test]
    fn agent_name_and_description_are_trimmed() {
        let (_, catalog) = setup_and_load_ok(
            r#"{"agents": [{"name": " planner ", "description": " Planner agent ", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );
        let spec = catalog.get("planner").unwrap();
        assert_eq!(spec.name, "planner");
        assert_eq!(spec.description, "Planner agent");
    }

    #[test]
    fn no_prompts_rejected() {
        let dir = create_temp_project();
        write_settings(
            dir.path(),
            r#"{"agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true}]}"#,
        );
        assert!(matches!(load_agent_catalog(dir.path()), Err(SettingsError::NoPrompts { .. })));
    }

    #[test]
    fn malformed_json_rejected() {
        let dir = create_temp_project();
        write_settings(dir.path(), "not valid json");
        assert!(matches!(load_agent_catalog(dir.path()), Err(SettingsError::ParseError(_))));
    }

    #[test]
    fn invalid_mcp_servers_path_rejected() {
        let (_, result) = setup_and_load(
            r#"{"mcpServers": ["nonexistent.json"], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );
        assert!(matches!(result, Err(SettingsError::InvalidMcpConfigPath { .. })));
    }

    #[test]
    fn invalid_agent_mcp_servers_path_rejected() {
        let (_, result) = setup_and_load(&agent_settings(r#""mcpServers": ["nonexistent.json"]"#));
        assert!(matches!(result, Err(SettingsError::InvalidMcpConfigPath { .. })));
    }

    #[test]
    fn valid_mcp_servers_path_accepted() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), ".aether/mcp/default.json", "{}");
        write_settings(
            dir.path(),
            r#"{"mcpServers": [".aether/mcp/default.json"], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let resolved = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(resolved.mcp_config_refs, vec![McpJsonFileRef::direct(dir.path().join(".aether/mcp/default.json"))]);
    }

    #[test]
    fn top_level_mcp_servers_array_parses_and_resolves_in_order() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), ".aether/mcp/a.json", "{}");
        write_file(dir.path(), ".aether/mcp/b.json", "{}");
        write_settings(
            dir.path(),
            r#"{"mcpServers": [".aether/mcp/a.json", ".aether/mcp/b.json"], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let resolved = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(
            resolved.mcp_config_refs,
            vec![
                McpJsonFileRef::direct(dir.path().join(".aether/mcp/a.json")),
                McpJsonFileRef::direct(dir.path().join(".aether/mcp/b.json")),
            ]
        );
    }

    #[test]
    fn top_level_mcp_servers_invalid_path_in_middle_of_array_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), "good.json", "{}");
        write_settings(
            dir.path(),
            r#"{"mcpServers": ["good.json", "bad.json"], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );

        let result = load_agent_catalog(dir.path());
        match result {
            Err(SettingsError::InvalidMcpConfigPath { path }) => assert_eq!(path, "bad.json"),
            other => panic!("expected InvalidMcpConfigPath for bad.json, got {other:?}"),
        }
    }

    #[test]
    fn agent_mcp_servers_array_parses() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), "a.json", "{}");
        write_file(dir.path(), "b.json", "{}");
        write_settings(
            dir.path(),
            r#"{"agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"], "mcpServers": ["a.json", "b.json"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let resolved = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(
            resolved.mcp_config_refs,
            vec![McpJsonFileRef::direct(dir.path().join("a.json")), McpJsonFileRef::direct(dir.path().join("b.json"))]
        );
    }

    #[test]
    fn agent_mcp_servers_overrides_inherited_array() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), "base.json", "{}");
        write_file(dir.path(), "override.json", "{}");
        write_settings(
            dir.path(),
            r#"{"mcpServers": ["base.json"], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"], "mcpServers": ["override.json"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let resolved = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(resolved.mcp_config_refs, vec![McpJsonFileRef::direct(dir.path().join("override.json"))]);
    }

    #[test]
    fn empty_mcp_servers_array_falls_back_to_cwd_mcp() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), "mcp.json", "{}");
        write_settings(
            dir.path(),
            r#"{"mcpServers": [], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let resolved = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(resolved.mcp_config_refs, vec![McpJsonFileRef::direct(dir.path().join("mcp.json"))]);
    }

    #[test]
    fn any_invalid_agent_entry_fails_catalog_load() {
        let (_, result) = setup_and_load(
            r#"{"agents": [
                {"name": "valid", "description": "Valid agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]},
                {"name": "invalid", "description": "Invalid agent", "model": "invalid:model", "userInvocable": true, "prompts": ["AGENTS.md"]}
            ]}"#,
        );
        assert!(matches!(result, Err(SettingsError::InvalidModel { .. })));
    }

    fn two_agent_json() -> &'static str {
        r#"{"agents": [
            {"name": "zebra", "description": "Z agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]},
            {"name": "alpha", "description": "A agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}
        ]}"#
    }

    #[test]
    fn preserves_authored_agent_order_and_lookup() {
        let (_, catalog) = setup_and_load_ok(two_agent_json());
        let names: Vec<_> = catalog.all().iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["zebra", "alpha"]); // not alphabetized
        assert_eq!(catalog.get("alpha").unwrap().name, "alpha");
        assert_eq!(catalog.get("zebra").unwrap().name, "zebra");
    }

    #[test]
    fn tools_filter_parsed_from_settings() {
        let (_, catalog) = setup_and_load_ok(
            r#"{"agents": [{"name": "researcher", "description": "Read-only agent", "model": "anthropic:claude-sonnet-4-5", "agentInvocable": true, "prompts": ["AGENTS.md"], "tools": {"allow": ["coding__grep", "coding__read_file"], "deny": ["coding__write*"]}}]}"#,
        );
        let spec = catalog.get("researcher").unwrap();
        assert_eq!(spec.tools.allow, vec!["coding__grep", "coding__read_file"]);
        assert_eq!(spec.tools.deny, vec!["coding__write*"]);
    }

    #[test]
    fn absent_tools_field_yields_default_filter() {
        let (_, catalog) = setup_and_load_ok(&agent_settings(""));
        let spec = catalog.get("planner").unwrap();
        assert!(spec.tools.allow.is_empty());
        assert!(spec.tools.deny.is_empty());
    }

    #[test]
    fn reserved_agent_name_rejected() {
        let (_, result) = setup_and_load(
            r#"{"agents": [{"name": "__default__", "description": "Sneaky agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );
        assert!(matches!(result, Err(SettingsError::ReservedAgentName { .. })));
    }

    #[test]
    fn mcp_server_entry_object_form_with_proxy() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), ".mcp.json", r#"{"servers": {}}"#);
        write_settings(
            dir.path(),
            r#"{"mcpServers": [{"path": ".mcp.json", "proxy": true}], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let resolved = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(resolved.mcp_config_refs, vec![McpJsonFileRef::proxied(dir.path().join(".mcp.json"))]);
    }

    #[test]
    fn mcp_server_entry_object_form_proxy_defaults_false() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), "a.json", r#"{"servers": {}}"#);
        write_settings(
            dir.path(),
            r#"{"mcpServers": [{"path": "a.json"}], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let resolved = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(resolved.mcp_config_refs, vec![McpJsonFileRef::direct(dir.path().join("a.json"))]);
    }

    #[test]
    fn mcp_server_entry_mixed_string_and_object() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), "direct.json", r#"{"servers": {}}"#);
        write_file(dir.path(), "proxied.json", r#"{"servers": {}}"#);
        write_settings(
            dir.path(),
            r#"{"mcpServers": ["direct.json", {"path": "proxied.json", "proxy": true}], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let resolved = catalog.resolve("planner", dir.path()).unwrap();
        assert_eq!(
            resolved.mcp_config_refs,
            vec![
                McpJsonFileRef::direct(dir.path().join("direct.json")),
                McpJsonFileRef::proxied(dir.path().join("proxied.json")),
            ]
        );
    }

    #[test]
    fn mcp_server_entry_object_form_invalid_path_rejected() {
        let (_, result) = setup_and_load(
            r#"{"mcpServers": [{"path": "nonexistent.json", "proxy": true}], "agents": [{"name": "planner", "description": "Planner agent", "model": "anthropic:claude-sonnet-4-5", "userInvocable": true, "prompts": ["AGENTS.md"]}]}"#,
        );
        assert!(matches!(result, Err(SettingsError::InvalidMcpConfigPath { .. })));
    }
}
