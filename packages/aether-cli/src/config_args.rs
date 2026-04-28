use std::path::{Path, PathBuf};

use aether_core::agent_spec::McpJsonFileRef;
use aether_core::mcp::McpConfigLayer;
use aether_project::{AgentCatalog, AgentCatalogSource, load_agent_catalog_from_source};

use crate::error::CliError;
use crate::runtime::McpConfigLayers;

#[derive(Clone, Debug, Default, clap::Args)]
pub struct SettingsSourceArgs {
    /// Inline `.aether/settings.json` equivalent as a JSON string.
    /// Conflicts with `--settings-file`.
    #[arg(long = "settings-json", conflicts_with = "settings_file")]
    pub settings_json: Option<String>,

    /// Path to a settings file to use instead of `cwd/.aether/settings.json`.
    /// Conflicts with `--settings-json`.
    #[arg(long = "settings-file", conflicts_with = "settings_json")]
    pub settings_file: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, clap::Args)]
pub struct McpConfigArgs {
    /// Path(s) to mcp.json. Pass multiple times to layer configs; later duplicate server names win.
    #[arg(long = "mcp-config", value_name = "PATH")]
    pub mcp_configs: Vec<PathBuf>,

    /// Inline MCP config JSON. Matches the mcp.json shape (`servers` or `mcpServers`).
    #[arg(long = "mcp-config-json", value_name = "JSON")]
    pub mcp_config_jsons: Vec<String>,
}

impl SettingsSourceArgs {
    pub fn into_catalog_source(self) -> Result<AgentCatalogSource, CliError> {
        if let Some(json) = self.settings_json {
            return AgentCatalogSource::from_settings_json(&json)
                .map_err(|e| CliError::AgentError(format!("Invalid --settings-json: {e}")));
        }
        if let Some(path) = self.settings_file {
            return AgentCatalogSource::from_settings_file(&path)
                .map_err(|e| CliError::AgentError(format!("Invalid --settings-file: {e}")));
        }
        Ok(AgentCatalogSource::ProjectFiles)
    }

    pub fn load_catalog(self, cwd: &Path) -> Result<AgentCatalog, CliError> {
        let source = self.into_catalog_source()?;
        load_agent_catalog_from_source(cwd, source).map_err(|e| CliError::AgentError(e.to_string()))
    }
}

impl McpConfigArgs {
    pub fn into_layers(self) -> McpConfigLayers {
        let layers = self
            .mcp_configs
            .into_iter()
            .map(|path| McpConfigLayer::File(McpJsonFileRef::direct(path)))
            .chain(self.mcp_config_jsons.into_iter().map(McpConfigLayer::Json))
            .collect();
        McpConfigLayers { layers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(flatten)]
        settings: SettingsSourceArgs,

        #[command(flatten)]
        mcp: McpConfigArgs,
    }

    #[test]
    fn settings_json_conflicts_with_settings_file() {
        let err = TestCli::try_parse_from(["test", "--settings-json", "{}", "--settings-file", "/tmp/settings.json"])
            .expect_err("settings-json and settings-file should conflict");
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn settings_json_alone_is_valid() {
        let cli = TestCli::try_parse_from(["test", "--settings-json", r#"{"agents":[]}"#]).expect("valid");
        assert!(cli.settings.settings_json.is_some());
        assert!(cli.settings.settings_file.is_none());
    }

    #[test]
    fn settings_file_alone_is_valid() {
        let cli = TestCli::try_parse_from(["test", "--settings-file", "/tmp/settings.json"]).expect("valid");
        assert!(cli.settings.settings_json.is_none());
        assert!(cli.settings.settings_file.is_some());
    }

    #[test]
    fn neither_defaults_to_project_files() {
        let cli = TestCli::try_parse_from(["test"]).expect("valid");
        let source = cli.settings.into_catalog_source().unwrap();
        assert!(matches!(source, AgentCatalogSource::ProjectFiles));
    }

    #[test]
    fn invalid_settings_json_returns_error() {
        let cli = TestCli::try_parse_from(["test", "--settings-json", "not json"]).expect("valid");
        let result = cli.settings.into_catalog_source();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("--settings-json"));
    }

    #[test]
    fn valid_settings_json_parses() {
        let cli = TestCli::try_parse_from([
            "test",
            "--settings-json",
            r#"{"agents":[{"name":"test","description":"Test","model":"anthropic:claude-sonnet-4-5","userInvocable":true,"prompts":[{"text":"hi"}]}]}"#,
        ])
        .expect("valid");
        let source = cli.settings.into_catalog_source().unwrap();
        assert!(matches!(source, AgentCatalogSource::Settings(_)));
    }

    #[test]
    fn mcp_config_args_layer_files_then_jsons() {
        let cli = TestCli::try_parse_from([
            "test",
            "--mcp-config",
            "/tmp/one.json",
            "--mcp-config-json",
            "{}",
            "--mcp-config",
            "/tmp/two.json",
        ])
        .expect("valid");
        let layers = cli.mcp.into_layers();
        assert_eq!(layers.layers.len(), 3);
        assert!(matches!(&layers.layers[0], McpConfigLayer::File(config) if config.path == Path::new("/tmp/one.json")));
        assert!(matches!(&layers.layers[1], McpConfigLayer::File(config) if config.path == Path::new("/tmp/two.json")));
        assert!(matches!(&layers.layers[2], McpConfigLayer::Json(json) if json == "{}"));
    }
}
