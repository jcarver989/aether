use clap::{Parser, Subcommand};
use std::process::ExitCode;
use tokio::runtime::Runtime;

use aether_cli::acp::AcpArgs;
use aether_cli::headless::HeadlessArgs;

#[derive(Parser)]
#[command(name = "aether")]
#[command(about = "Aether AI coding agent")]
struct Cli {
    /// Run the CLI inside a Docker container for filesystem isolation
    #[arg(long, global = true)]
    sandbox: bool,

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

    if cli.sandbox {
        return aether_cli::sandbox::exec_in_container();
    }

    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let result: Result<ExitCode, String> = match cli.command {
        Command::Headless(args) => rt
            .block_on(aether_cli::headless::run_headless(args))
            .map_err(|e| e.to_string()),

        Command::Acp(args) => rt
            .block_on(aether_cli::acp::run_acp(args))
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
