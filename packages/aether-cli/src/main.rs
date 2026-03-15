use clap::{Parser, Subcommand};
use std::process::ExitCode;
use tokio::runtime::Runtime;

use aether_cli::acp::{AcpArgs, run_acp};
use aether_cli::headless::{HeadlessArgs, run_headless};
use aether_cli::prompt_cmd::{PromptArgs, run_prompt};

#[derive(Parser)]
#[command(name = "aether")]
#[command(about = "Aether AI coding agent")]
struct Cli {
    /// Run inside a Docker sandbox using the given image
    #[arg(long, global = true)]
    sandbox_image: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a single prompt headlessly
    Headless(HeadlessArgs),
    /// Start the ACP server
    Acp(AcpArgs),
    /// Print the fully assembled system prompt (for debugging)
    Prompt(PromptArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(image) = cli.sandbox_image {
        return aether_cli::sandbox::exec_in_container(&image);
    }

    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let result: Result<ExitCode, String> = match cli.command {
        Command::Headless(args) => rt.block_on(run_headless(args)).map_err(|e| e.to_string()),

        Command::Acp(args) => rt
            .block_on(run_acp(args))
            .map(|()| ExitCode::SUCCESS)
            .map_err(|e| e.to_string()),

        Command::Prompt(args) => rt
            .block_on(run_prompt(args))
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
