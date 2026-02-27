use aether_bin::cli::{Cli, run};
use clap::Parser;
use std::process::ExitCode;
use tokio::runtime::Runtime;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let rt = Runtime::new().expect("Failed to create tokio runtime");
    match rt.block_on(run(cli)) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}
