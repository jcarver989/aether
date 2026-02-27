use clap::{Parser, Subcommand};
use std::process::ExitCode;
use tokio::runtime::Runtime;

use aether_bin::acp::AcpArgs;
use aether_bin::headless::HeadlessArgs;

#[derive(Parser)]
#[command(name = "aether")]
#[command(about = "Aether AI coding agent")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a single prompt headlessly
    Headless(HeadlessArgs),
    /// Start the ACP server
    Acp(AcpArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let result: Result<ExitCode, String> = match cli.command {
        Command::Headless(args) => rt
            .block_on(aether_bin::headless::run_headless(args))
            .map_err(|e| e.to_string()),

        Command::Acp(args) => rt
            .block_on(aether_bin::acp::run_acp(args))
            .map(|()| ExitCode::SUCCESS)
            .map_err(|e| e.to_string()),
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}
