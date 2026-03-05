use clap::{Parser, Subcommand};
use std::process::ExitCode;
use tokio::runtime::Runtime;

use aether_cli::acp::AcpArgs;
use aether_cli::auth::AuthArgs;
use aether_cli::headless::HeadlessArgs;

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
    /// Authenticate with a provider (e.g. `aether auth codex`)
    Auth(AuthArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let result: Result<ExitCode, String> = match cli.command {
        Command::Headless(args) => rt
            .block_on(aether_cli::headless::run_headless(args))
            .map_err(|e| e.to_string()),

        Command::Acp(args) => rt
            .block_on(aether_cli::acp::run_acp(args))
            .map(|()| ExitCode::SUCCESS)
            .map_err(|e| e.to_string()),

        Command::Auth(args) => rt
            .block_on(aether_cli::auth::run_auth(args))
            .map(|()| ExitCode::SUCCESS),
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}
