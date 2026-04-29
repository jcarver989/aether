use crate::catalog::AgentCatalog;
use crate::error::SettingsError;
use aether_core::agent_spec::{AgentSpec, AgentSpecExposure, McpConfigSource, ToolFilter};
use aether_core::core::Prompt;
use glob::glob;
use llm::{LlmModel, ReasoningEffort};
use mcp_utils::client::{RawMcpConfig, RawMcpServerConfig};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AetherConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[schemars(length(min = 1))]
    pub agents: Vec<AgentConfig>,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(transform = require_agent_invocation_surface_schema)]
pub struct AgentConfig {
    #[schemars(length(min = 1))]
    pub name: String,
    #[schemars(length(min = 1))]
    pub description: String,
    #[schemars(length(min = 1))]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default)]
    pub user_invocable: bool,
    #[serde(default)]
    pub agent_invocable: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(min = 1))]
    pub prompts: Vec<PromptSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp: Vec<McpConfigSourceConfig>,
    #[serde(default, skip_serializing_if = "ToolFilter::is_empty")]
    pub tools: ToolFilter,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum PromptSource {
    Text { text: String },
    File { path: String },
    Glob { pattern: String },
}

impl PromptSource {
    pub fn file(path: impl Into<String>) -> Self {
        Self::File { path: path.into() }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            Self::File { path } => Some(path.as_str()),
            Self::Glob { pattern } => Some(pattern.as_str()),
            Self::Text { .. } => None,
        }
    }
}

impl From<&str> for PromptSource {
    fn from(value: &str) -> Self {
        Self::file(value)
    }
}

impl From<String> for PromptSource {
    fn from(value: String) -> Self {
        Self::file(value)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum McpConfigSourceConfig {
    File {
        path: String,
        #[serde(default)]
        proxy: bool,
    },
    Inline {
        servers: BTreeMap<String, RawMcpServerConfig>,
    },
}

impl McpConfigSourceConfig {
    pub fn file(path: impl Into<String>) -> Self {
        Self::File { path: path.into(), proxy: false }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            Self::File { path, .. } => Some(path.as_str()),
            Self::Inline { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AetherConfigSource {
    ProjectFiles,
    File(PathBuf),
    Json(String),
    Value(AetherConfig),
}

impl AetherConfig {
    pub fn builtin_fallback() -> Self {
        Self {
            agent: Some("default".to_string()),
            agents: vec![AgentConfig {
                name: "default".to_string(),
                description: "Default agent".to_string(),
                model: "anthropic:claude-sonnet-4-5".to_string(),
                user_invocable: true,
                prompts: vec![PromptSource::Text { text: "You are Aether, an AI coding assistant.".to_string() }],
                ..AgentConfig::default()
            }],
        }
    }
}

pub fn load_aether_config(project_root: &Path) -> Result<AetherConfig, SettingsError> {
    load_aether_config_from_source(project_root, AetherConfigSource::ProjectFiles)
}

pub fn load_aether_config_from_source(
    project_root: &Path,
    source: AetherConfigSource,
) -> Result<AetherConfig, SettingsError> {
    match source {
        AetherConfigSource::ProjectFiles => {
            let settings_path = project_root.join(".aether/settings.json");
            match std::fs::read_to_string(&settings_path) {
                Ok(content) if content.trim().is_empty() => Ok(AetherConfig::builtin_fallback()),
                Ok(content) => parse_config(&content),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AetherConfig::builtin_fallback()),
                Err(e) => Err(SettingsError::IoError(format!("Failed to read {}: {}", settings_path.display(), e))),
            }
        }
        AetherConfigSource::File(path) => {
            let path = resolve_path(project_root, &path);
            let content = std::fs::read_to_string(&path)
                .map_err(|e| SettingsError::IoError(format!("Failed to read {}: {}", path.display(), e)))?;
            parse_config(&content)
        }
        AetherConfigSource::Json(json) => parse_config(&json),
        AetherConfigSource::Value(config) => Ok(config),
    }
}

pub fn load_agent_catalog(project_root: &Path) -> Result<AgentCatalog, SettingsError> {
    load_agent_catalog_from_source(project_root, AetherConfigSource::ProjectFiles)
}

pub fn load_agent_catalog_from_source(
    project_root: &Path,
    source: AetherConfigSource,
) -> Result<AgentCatalog, SettingsError> {
    let config = load_aether_config_from_source(project_root, source)?;
    resolve_config(project_root, config)
}

pub fn resolve_config(project_root: &Path, config: AetherConfig) -> Result<AgentCatalog, SettingsError> {
    validate_config_selector(&config)?;
    let selected_agent = config.agent.as_deref().map(str::trim).filter(|name| !name.is_empty()).map(str::to_string);
    let mut seen_names = HashSet::new();
    let mut specs = Vec::with_capacity(config.agents.len());

    for (index, entry) in config.agents.into_iter().enumerate() {
        specs.push(resolve_agent_entry(project_root, entry, index, &mut seen_names)?);
    }

    Ok(AgentCatalog::new(project_root.to_path_buf(), specs, selected_agent))
}

fn require_agent_invocation_surface_schema(schema: &mut schemars::Schema) {
    schema.insert(
        "anyOf".to_string(),
        serde_json::json!([
            { "required": ["userInvocable"], "properties": { "userInvocable": { "const": true } } },
            { "required": ["agentInvocable"], "properties": { "agentInvocable": { "const": true } } }
        ]),
    );
}

fn parse_config(content: &str) -> Result<AetherConfig, SettingsError> {
    serde_json::from_str(content).map_err(|e| SettingsError::ParseError(e.to_string()))
}

fn validate_config_selector(config: &AetherConfig) -> Result<(), SettingsError> {
    if config.agents.is_empty() {
        return Err(SettingsError::EmptyAgents);
    }

    if let Some(agent) = config.agent.as_deref() {
        let selector = agent.trim();
        let Some(entry) = config.agents.iter().find(|entry| entry.name.trim() == selector) else {
            return Err(SettingsError::InvalidAgentSelector { name: selector.to_string() });
        };
        if !entry.user_invocable {
            return Err(SettingsError::NonUserInvocableAgentSelector { name: selector.to_string() });
        }
    }

    Ok(())
}

fn resolve_agent_entry(
    project_root: &Path,
    entry: AgentConfig,
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
    if entry.prompts.is_empty() {
        return Err(SettingsError::NoPrompts { agent: name.clone() });
    }

    let prompts = resolve_prompts(project_root, &name, &entry.prompts)?;
    let mcp_config_sources = resolve_mcp_config_sources(project_root, &entry.mcp)?;

    Ok(AgentSpec {
        name,
        description,
        model,
        reasoning_effort: entry.reasoning_effort,
        prompts,
        mcp_config_sources,
        exposure: AgentSpecExposure { user_invocable: entry.user_invocable, agent_invocable: entry.agent_invocable },
        tools: entry.tools,
    })
}

fn resolve_prompts(project_root: &Path, agent: &str, prompts: &[PromptSource]) -> Result<Vec<Prompt>, SettingsError> {
    prompts
        .iter()
        .map(|source| match source {
            PromptSource::Text { text } => Ok(Prompt::text(text)),
            PromptSource::File { path } => validate_prompt_file(project_root, agent, path)
                .map(|()| Prompt::file(path).with_cwd(project_root.to_path_buf())),
            PromptSource::Glob { pattern } => validate_prompt_glob(project_root, agent, pattern)
                .map(|()| Prompt::from_globs(vec![pattern.clone()], project_root.to_path_buf())),
        })
        .collect()
}

fn resolve_mcp_config_sources(
    project_root: &Path,
    entries: &[McpConfigSourceConfig],
) -> Result<Vec<McpConfigSource>, SettingsError> {
    entries
        .iter()
        .map(|entry| match entry {
            McpConfigSourceConfig::File { path, proxy } => {
                let full_path = resolve_path(project_root, Path::new(path));
                if full_path.is_file() {
                    Ok(McpConfigSource::file(full_path, *proxy))
                } else {
                    Err(SettingsError::InvalidMcpConfigPath { path: path.clone() })
                }
            }
            McpConfigSourceConfig::Inline { servers } => {
                Ok(McpConfigSource::Inline(RawMcpConfig { servers: servers.clone() }))
            }
        })
        .collect()
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

fn validate_prompt_file(project_root: &Path, agent: &str, path: &str) -> Result<(), SettingsError> {
    let full_path = resolve_path(project_root, Path::new(path));
    if full_path.is_file() {
        Ok(())
    } else {
        Err(SettingsError::ZeroMatchPrompt { agent: agent.to_string(), pattern: path.to_string() })
    }
}

fn validate_prompt_glob(project_root: &Path, agent: &str, pattern: &str) -> Result<(), SettingsError> {
    let full_pattern = if Path::new(pattern).is_absolute() {
        pattern.to_string()
    } else {
        project_root.join(pattern).to_string_lossy().to_string()
    };

    let has_file_match = glob(&full_pattern)
        .map_err(|e| SettingsError::InvalidGlobPattern {
            agent: agent.to_string(),
            pattern: pattern.to_string(),
            error: e.to_string(),
        })?
        .filter_map(Result::ok)
        .any(|path| path.is_file());

    if has_file_match {
        Ok(())
    } else {
        Err(SettingsError::ZeroMatchPrompt { agent: agent.to_string(), pattern: pattern.to_string() })
    }
}

fn resolve_path(project_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { project_root.join(path) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(dir: &Path, path: &str, content: &str) {
        let full = dir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    fn valid_agent(name: &str) -> AgentConfig {
        AgentConfig {
            name: name.to_string(),
            description: format!("{name} agent"),
            model: "anthropic:claude-sonnet-4-5".to_string(),
            user_invocable: true,
            prompts: vec![PromptSource::file("PROMPT.md")],
            ..AgentConfig::default()
        }
    }

    #[test]
    fn resolves_selected_agent() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "PROMPT.md", "Be helpful");
        let config =
            AetherConfig { agent: Some("beta".to_string()), agents: vec![valid_agent("alpha"), valid_agent("beta")] };

        let catalog = resolve_config(dir.path(), config).unwrap();

        assert_eq!(catalog.default_agent().map(|spec| spec.name.as_str()), Some("beta"));
    }

    #[test]
    fn rejects_selected_agent_that_is_not_user_invocable() {
        let mut internal = valid_agent("internal");
        internal.user_invocable = false;
        internal.agent_invocable = true;
        let config = AetherConfig { agent: Some("internal".to_string()), agents: vec![internal] };

        let err = resolve_config(Path::new("/tmp"), config).unwrap_err();

        assert!(matches!(err, SettingsError::NonUserInvocableAgentSelector { .. }));
    }

    #[test]
    fn config_file_paths_are_project_relative() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "PROMPT.md", "Be helpful");
        write_file(
            dir.path(),
            "nested/config.json",
            r#"{"agents":[{"name":"alpha","description":"Alpha","model":"anthropic:claude-sonnet-4-5","userInvocable":true,"prompts":[{"type":"file","path":"PROMPT.md"}]}]}"#,
        );

        let catalog =
            load_agent_catalog_from_source(dir.path(), AetherConfigSource::File(PathBuf::from("nested/config.json")))
                .unwrap();

        assert_eq!(catalog.all()[0].name, "alpha");
    }

    #[test]
    fn resolves_inline_mcp_config() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "PROMPT.md", "Be helpful");
        let config = AetherConfig {
            agent: None,
            agents: vec![AgentConfig {
                mcp: vec![McpConfigSourceConfig::Inline { servers: BTreeMap::new() }],
                ..valid_agent("alpha")
            }],
        };

        let catalog = resolve_config(dir.path(), config).unwrap();
        let spec = catalog.resolve("alpha", dir.path()).unwrap();

        assert_eq!(spec.mcp_config_sources.len(), 1);
        assert!(matches!(spec.mcp_config_sources[0], McpConfigSource::Inline(_)));
    }
}
