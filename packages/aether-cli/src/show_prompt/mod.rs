mod run;

use std::path::PathBuf;

#[derive(clap::Args)]
pub struct PromptArgs {
    /// Working directory
    #[arg(short = 'C', long = "cwd", default_value = ".")]
    pub cwd: PathBuf,

    /// Path to mcp.json (auto-detected if omitted)
    #[arg(long = "mcp-config")]
    pub mcp_config: Option<PathBuf>,

    /// Additional system prompt
    #[arg(long = "system-prompt")]
    pub system_prompt: Option<String>,
}

pub use run::run_prompt;
