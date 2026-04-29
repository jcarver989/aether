use aether_core::agent_spec::McpConfigSource;
use aether_project::AetherConfigSource;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, clap::Args)]
pub struct ConfigSourceArgs {
    #[arg(long = "config-json", conflicts_with = "config_file")]
    pub config_json: Option<String>,

    #[arg(long = "config-file", conflicts_with = "config_json")]
    pub config_file: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, clap::Args)]
pub struct McpConfigArgs {
    #[arg(long = "mcp-config", value_name = "PATH")]
    pub mcp_configs: Vec<PathBuf>,

    #[arg(long = "mcp-config-json", value_name = "JSON")]
    pub mcp_config_jsons: Vec<String>,
}

impl ConfigSourceArgs {
    pub fn source(&self) -> AetherConfigSource {
        if let Some(json) = &self.config_json {
            AetherConfigSource::Json(json.clone())
        } else if let Some(path) = &self.config_file {
            AetherConfigSource::File(path.clone())
        } else {
            AetherConfigSource::ProjectFiles
        }
    }
}

impl McpConfigArgs {
    pub fn sources(&self, project_root: &Path) -> Vec<McpConfigSource> {
        self.mcp_configs
            .iter()
            .map(|path| resolve_path(project_root, path))
            .map(McpConfigSource::direct)
            .chain(self.mcp_config_jsons.iter().cloned().map(McpConfigSource::Json))
            .collect()
    }
}

fn resolve_path(project_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { project_root.join(path) }
}
