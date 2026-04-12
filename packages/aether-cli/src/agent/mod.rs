mod list;
mod new;
pub mod new_agent_wizard;
mod remove;

use std::path::PathBuf;

#[derive(clap::Subcommand)]
pub enum AgentCommand {
    /// Create a new agent
    New(NewArgs),
    /// List all agents in the project
    List(ListArgs),
    /// Remove an agent from the project
    Remove(RemoveArgs),
}

#[derive(clap::Args)]
pub struct NewArgs {
    /// Directory to initialize (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[derive(clap::Args)]
pub struct ListArgs {
    /// Project directory (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[derive(clap::Args)]
pub struct RemoveArgs {
    /// Name of the agent to remove
    pub name: String,
    /// Project directory (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

pub use list::run_list;
pub use new::run_new;
pub use new_agent_wizard::{NewAgentOutcome, should_run_onboarding};
pub use remove::run_remove;
