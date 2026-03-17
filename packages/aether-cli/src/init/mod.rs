mod run;

use std::path::PathBuf;

#[derive(clap::Args)]
pub struct InitArgs {
    /// Directory to initialize (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

pub use run::run_init;
