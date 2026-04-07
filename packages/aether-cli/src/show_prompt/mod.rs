mod run;

use std::path::PathBuf;

#[derive(clap::Args)]
pub struct PromptArgs {
    /// Working directory
    #[arg(short = 'C', long = "cwd", default_value = ".")]
    pub cwd: PathBuf,

    /// Path(s) to mcp.json. Pass multiple times to layer configs (last wins on collisions).
    /// If omitted, paths from settings.json `mcpServers` are used (or `cwd/mcp.json` is auto-detected).
    #[arg(long = "mcp-config")]
    pub mcp_configs: Vec<PathBuf>,

    /// Additional system prompt
    #[arg(long = "system-prompt")]
    pub system_prompt: Option<String>,

    /// Named agent to inspect (defaults to first user-invocable agent)
    #[arg(short = 'a', long = "agent")]
    pub agent: Option<String>,
}

pub use run::run_prompt;
