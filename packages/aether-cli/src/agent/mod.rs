mod new;

use std::path::PathBuf;

#[derive(clap::Subcommand)]
pub enum AgentCommand {
    /// Create a new agent
    New(NewArgs),
}

#[derive(clap::Args)]
pub struct NewArgs {
    /// Directory to initialize (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

pub use new::run_new;
