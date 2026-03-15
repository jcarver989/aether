//! Settings file parsing and validation.

use crate::error::SettingsError;
use aether_core::agent_spec::{AgentSpec, AgentSpecExposure};
use aether_core::core::Prompt;
use glob::glob;
use llm::{LlmModel, ReasoningEffort};
use std::collections::HashSet;
use std::path::Path;

/// Settings DTO for deserializing `.aether/settings.json`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Settings {
    /// Inherited prompts for all agents.
    prompts: Vec<String>,
    /// Path to inherited MCP config for all agents.
    mcp_servers: Option<String>,
    /// The canonical authored agent registry.
    agents: Vec<AgentEntry>,
}

/// Agent entry DTO for deserializing from settings.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentEntry {
    name: String,
    description: String,
    model: String,
    #[serde(default)]
    reasoning_effort: Option<String>,
    #[serde(default)]
    user_invocable: bool,
    #[serde(default)]
    agent_invocable: bool,
    #[serde(default)]
    prompts: Vec<String>,
    mcp_servers: Option<String>,
}

/// Load and resolve the agent catalog from a project root.
///
/// If `.aether/settings.json` is absent, returns a valid empty catalog.
/// If the settings file is malformed or contains invalid entries, returns an error.
pub fn load_agent_catalog(
    project_root: &Path,
) -> Result<super::catalog::AgentCatalog, SettingsError> {
    let settings_path = project_root.join(".aether/settings.json");

    let settings = match std::fs::read_to_string(&settings_path) {
        Ok(content) => {
            if content.trim().is_empty() {
                Settings::default()
            } else {
                serde_json::from_str(&content)
                    .map_err(|e| SettingsError::ParseError(e.to_string()))?
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(super::catalog::AgentCatalog::empty(
                project_root.to_path_buf(),
            ));
        }
        Err(e) => {
            return Err(SettingsError::IoError(format!(
                "Failed to read {}: {}",
                settings_path.display(),
                e
            )));
        }
    };

    resolve_settings(project_root, settings)
}

/// Resolve settings into a catalog of agent specs.
fn resolve_settings(
    project_root: &Path,
    settings: Settings,
) -> Result<super::catalog::AgentCatalog, SettingsError> {
    let Settings {
        prompts: inherited_patterns,
        mcp_servers,
        agents,
    } = settings;

    validate_prompt_entries(project_root, &inherited_patterns, None)?;
    let inherited_mcp_config_path = resolve_mcp_config_path(project_root, mcp_servers.as_deref())?;
    let inherited_prompts = build_inherited_prompts(&inherited_patterns, project_root);

    let mut seen_names = HashSet::new();
    let mut specs = Vec::with_capacity(agents.len());

    for (index, entry) in agents.into_iter().enumerate() {
        specs.push(resolve_agent_entry(
            project_root,
            &inherited_prompts,
            entry,
            index,
            &mut seen_names,
        )?);
    }

    Ok(super::catalog::AgentCatalog::new(
        project_root.to_path_buf(),
        inherited_prompts,
        inherited_mcp_config_path,
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
        return Err(SettingsError::MissingField {
            agent: name.clone(),
            field: "description".to_string(),
        });
    }

    let model = parse_model(&name, &entry.model)?;
    let reasoning_effort = parse_reasoning_effort(&name, entry.reasoning_effort)?;

    if !entry.user_invocable && !entry.agent_invocable {
        return Err(SettingsError::NoInvocationSurface {
            agent: name.clone(),
        });
    }

    validate_prompt_entries(project_root, &entry.prompts, Some(&name))?;

    if inherited_prompts.is_empty() && entry.prompts.is_empty() {
        return Err(SettingsError::NoPrompts {
            agent: name.clone(),
        });
    }

    let mcp_config_path =
        resolve_mcp_config_path(project_root, entry.mcp_servers.as_deref())?;

    let mut prompts = Vec::with_capacity(inherited_prompts.len() + entry.prompts.len());
    prompts.extend_from_slice(inherited_prompts);
    for pattern in &entry.prompts {
        prompts.push(Prompt::from_globs(
            vec![pattern.clone()],
            project_root.to_path_buf(),
        ));
    }

    Ok(AgentSpec {
        name,
        description,
        model,
        reasoning_effort,
        prompts,
        mcp_config_path,
        exposure: AgentSpecExposure {
            user_invocable: entry.user_invocable,
            agent_invocable: entry.agent_invocable,
        },
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

fn parse_reasoning_effort(
    agent: &str,
    reasoning_effort: Option<String>,
) -> Result<Option<ReasoningEffort>, SettingsError> {
    match reasoning_effort {
        None => Ok(None),
        Some(value) => {
            let value = value.trim();
            if value.is_empty() {
                return Ok(None);
            }

            ReasoningEffort::parse(value).map_err(|error| SettingsError::InvalidReasoningEffort {
                agent: agent.to_string(),
                effort: value.to_string(),
                error,
            })
        }
    }
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

fn resolve_mcp_config_path(
    project_root: &Path,
    mcp_path: Option<&str>,
) -> Result<Option<std::path::PathBuf>, SettingsError> {
    match mcp_path {
        None => Ok(None),
        Some(path) => {
            let full_path = project_root.join(path);
            if full_path.is_file() {
                Ok(Some(full_path))
            } else {
                Err(SettingsError::InvalidMcpConfigPath {
                    path: path.to_string(),
                })
            }
        }
    }
}

/// Validate that a prompt entry resolves to at least one file.
fn validate_prompt_entry(
    project_root: &Path,
    pattern: &str,
    agent_name: Option<&str>,
) -> Result<(), SettingsError> {
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
                SettingsError::InvalidInheritedGlobPattern {
                    pattern: pattern.to_string(),
                    error: e.to_string(),
                }
            }
        })?
        .filter_map(Result::ok)
        .any(|path| path.is_file());

    if has_file_match {
        Ok(())
    } else if let Some(agent) = agent_name {
        Err(SettingsError::ZeroMatchPrompt {
            agent: agent.to_string(),
            pattern: pattern.to_string(),
        })
    } else {
        Err(SettingsError::ZeroMatchInheritedPrompt {
            pattern: pattern.to_string(),
        })
    }
}

/// Build the inherited prompts from patterns.
///
/// Each pattern becomes one `Prompt::PromptGlobs` value.
fn build_inherited_prompts(patterns: &[String], project_root: &Path) -> Vec<Prompt> {
    patterns
        .iter()
        .map(|pattern| Prompt::from_globs(vec![pattern.clone()], project_root.to_path_buf()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn missing_settings_yields_empty_catalog() {
        let dir = create_temp_project();
        let catalog = load_agent_catalog(dir.path()).unwrap();
        assert!(catalog.all().is_empty());
    }

    #[test]
    fn valid_agent_with_both_exposure_flags() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "agentInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        assert_eq!(catalog.all().len(), 1);
        let spec = catalog.get("planner").unwrap();
        assert!(spec.exposure.user_invocable);
        assert!(spec.exposure.agent_invocable);
    }

    #[test]
    fn valid_user_only_spec() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "agentInvocable": false,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let spec = catalog.get("planner").unwrap();
        assert!(spec.exposure.user_invocable);
        assert!(!spec.exposure.agent_invocable);
    }

    #[test]
    fn valid_agent_only_spec() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": false,
                    "agentInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let spec = catalog.get("planner").unwrap();
        assert!(!spec.exposure.user_invocable);
        assert!(spec.exposure.agent_invocable);
    }

    #[test]
    fn invalid_model_string_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "invalid:model",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::InvalidModel { .. })));
    }

    #[test]
    fn alloy_model_string_is_accepted() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "alloy",
                    "description": "Alloy agent",
                    "model": "anthropic:claude-sonnet-4-5,deepseek:deepseek-chat",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let spec = catalog.get("alloy").unwrap();
        assert_eq!(
            spec.model.to_string(),
            "anthropic:claude-sonnet-4-5,deepseek:deepseek-chat"
        );
    }

    #[test]
    fn alloy_model_string_with_unknown_member_is_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "alloy",
                    "description": "Alloy agent",
                    "model": "anthropic:claude-sonnet-4-5,mystery:nope",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::InvalidModel { .. })));
    }

    #[test]
    fn invalid_reasoning_effort_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "reasoningEffort": "invalid",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(
            result,
            Err(SettingsError::InvalidReasoningEffort { .. })
        ));
    }

    #[test]
    fn duplicate_agent_names_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [
                    {
                        "name": "planner",
                        "description": "First planner",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    },
                    {
                        "name": "planner",
                        "description": "Second planner",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    }
                ]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(
            result,
            Err(SettingsError::DuplicateAgentName { .. })
        ));
    }

    #[test]
    fn top_level_prompts_inherited_by_all_agents() {
        let dir = create_temp_project();
        write_file(dir.path(), "BASE.md", "Base instructions");
        write_file(dir.path(), "AGENTS.md", "Agent instructions");
        write_settings(
            dir.path(),
            r#"{
                "prompts": ["BASE.md"],
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "agentInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let spec = catalog.get("planner").unwrap();
        // Should have 2 prompts: 1 inherited + 1 local
        assert_eq!(spec.prompts.len(), 2);
    }

    #[test]
    fn one_prompt_globs_per_entry() {
        let dir = create_temp_project();
        write_file(dir.path(), "a.md", "A");
        write_file(dir.path(), "b.md", "B");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["a.md", "b.md"]
                }]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let spec = catalog.get("planner").unwrap();
        // Should have 2 PromptGlobs entries, not 1 combined
        assert_eq!(spec.prompts.len(), 2);
    }

    #[test]
    fn zero_match_prompt_rejected() {
        let dir = create_temp_project();
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["nonexistent.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::ZeroMatchPrompt { .. })));
    }

    #[test]
    fn prompt_matching_only_directories_is_rejected() {
        let dir = create_temp_project();
        std::fs::create_dir_all(dir.path().join("prompts")).unwrap();
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["prompts/*"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::ZeroMatchPrompt { .. })));
    }

    #[test]
    fn no_invocation_surface_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": false,
                    "agentInvocable": false,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(
            result,
            Err(SettingsError::NoInvocationSurface { .. })
        ));
    }

    #[test]
    fn empty_agent_name_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "",
                    "description": "No name",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::EmptyAgentName { .. })));
    }

    #[test]
    fn empty_description_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::MissingField { .. })));
    }

    #[test]
    fn whitespace_only_agent_name_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "   ",
                    "description": "Has spaces only",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::EmptyAgentName { .. })));
    }

    #[test]
    fn whitespace_only_description_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "   ",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::MissingField { .. })));
    }

    #[test]
    fn duplicate_agent_names_after_trim_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [
                    {
                        "name": "planner",
                        "description": "First",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    },
                    {
                        "name": " planner ",
                        "description": "Second",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    }
                ]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(
            result,
            Err(SettingsError::DuplicateAgentName { .. })
        ));
    }

    #[test]
    fn agent_name_and_description_are_trimmed() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": " planner ",
                    "description": " Planner agent ",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let spec = catalog.get("planner").unwrap();
        assert_eq!(spec.name, "planner");
        assert_eq!(spec.description, "Planner agent");
    }

    #[test]
    fn no_prompts_rejected() {
        let dir = create_temp_project();
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::NoPrompts { .. })));
    }

    #[test]
    fn malformed_json_rejected() {
        let dir = create_temp_project();
        write_settings(dir.path(), "not valid json");

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::ParseError(_))));
    }

    #[test]
    fn invalid_mcp_servers_path_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "mcpServers": "nonexistent.json",
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(
            result,
            Err(SettingsError::InvalidMcpConfigPath { .. })
        ));
    }

    #[test]
    fn invalid_agent_mcp_servers_path_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"],
                    "mcpServers": "nonexistent.json"
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(
            result,
            Err(SettingsError::InvalidMcpConfigPath { .. })
        ));
    }

    #[test]
    fn valid_mcp_servers_path_accepted() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_file(dir.path(), ".aether/mcp/default.json", "{}");
        write_settings(
            dir.path(),
            r#"{
                "mcpServers": ".aether/mcp/default.json",
                "agents": [{
                    "name": "planner",
                    "description": "Planner agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let spec = catalog.resolve("planner", dir.path()).unwrap();
        assert!(spec.mcp_config_path.is_some());
    }

    #[test]
    fn any_invalid_agent_entry_fails_catalog_load() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [
                    {
                        "name": "valid",
                        "description": "Valid agent",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    },
                    {
                        "name": "invalid",
                        "description": "Invalid agent",
                        "model": "invalid:model",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    }
                ]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(result, Err(SettingsError::InvalidModel { .. })));
    }

    #[test]
    fn preserves_authored_agent_order() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [
                    {
                        "name": "zebra",
                        "description": "Z agent",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    },
                    {
                        "name": "alpha",
                        "description": "A agent",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    }
                ]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let names: Vec<_> = catalog.all().iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["zebra", "alpha"]);
    }

    #[test]
    fn get_uses_exact_name_lookup_independent_of_authored_order() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [
                    {
                        "name": "zebra",
                        "description": "Z agent",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    },
                    {
                        "name": "alpha",
                        "description": "A agent",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    }
                ]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        assert_eq!(catalog.get("alpha").unwrap().name, "alpha");
        assert_eq!(catalog.get("zebra").unwrap().name, "zebra");
    }

    #[test]
    fn authored_order_is_not_alphabetized() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [
                    {
                        "name": "zebra",
                        "description": "Z agent",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    },
                    {
                        "name": "alpha",
                        "description": "A agent",
                        "model": "anthropic:claude-sonnet-4-5",
                        "userInvocable": true,
                        "prompts": ["AGENTS.md"]
                    }
                ]
            }"#,
        );

        let catalog = load_agent_catalog(dir.path()).unwrap();
        let names: Vec<_> = catalog.all().iter().map(|s| s.name.as_str()).collect();
        assert_ne!(names, vec!["alpha", "zebra"]);
    }

    #[test]
    fn reserved_agent_name_rejected() {
        let dir = create_temp_project();
        write_file(dir.path(), "AGENTS.md", "Be helpful");
        write_settings(
            dir.path(),
            r#"{
                "agents": [{
                    "name": "__default__",
                    "description": "Sneaky agent",
                    "model": "anthropic:claude-sonnet-4-5",
                    "userInvocable": true,
                    "prompts": ["AGENTS.md"]
                }]
            }"#,
        );

        let result = load_agent_catalog(dir.path());
        assert!(matches!(
            result,
            Err(SettingsError::ReservedAgentName { .. })
        ));
    }
}
