use aether_cli::acp::{AcpArgs, run_acp};
use aether_cli::agent::{AgentCommand, NewAgentOutcome, NewArgs, run_list, run_new, run_remove, should_run_onboarding};
use aether_cli::headless::{HeadlessArgs, run_headless};
use aether_cli::show_prompt::{PromptArgs, run_prompt};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(name = "aether")]
#[command(about = "Aether AI coding agent")]
#[command(version)]
struct Cli {
    /// Run inside a Docker sandbox using the given image
    #[arg(long, global = true)]
    sandbox_image: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run a single prompt headlessly
    Headless(HeadlessArgs),
    /// Start the ACP server
    Acp(AcpArgs),
    /// Print the fully assembled system prompt (for debugging)
    ShowPrompt(PromptArgs),
    /// Manage agents
    #[command(subcommand)]
    Agent(AgentCommand),
    /// Start the LSP daemon (used internally)
    #[command(hide = true)]
    Lspd(aether_lspd::LspdArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(image) = cli.sandbox_image {
        return aether_cli::sandbox::exec_in_container(&image);
    }

    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let result: Result<ExitCode, String> = match cli.command {
        Some(Command::Headless(args)) => rt.block_on(run_headless(args)).map_err(|e| e.to_string()),

        Some(Command::Acp(args)) => rt.block_on(run_acp(args)).map(|()| ExitCode::SUCCESS).map_err(|e| e.to_string()),

        Some(Command::ShowPrompt(args)) => {
            rt.block_on(run_prompt(args)).map(|()| ExitCode::SUCCESS).map_err(|e| e.to_string())
        }

        Some(Command::Agent(AgentCommand::New(args))) => {
            rt.block_on(run_new(args)).map(|_| ExitCode::SUCCESS).map_err(|e| e.to_string())
        }

        Some(Command::Agent(AgentCommand::List(args))) => {
            run_list(args).map(|()| ExitCode::SUCCESS).map_err(|e| e.to_string())
        }

        Some(Command::Agent(AgentCommand::Remove(args))) => {
            run_remove(args).map(|()| ExitCode::SUCCESS).map_err(|e| e.to_string())
        }

        Some(Command::Lspd(args)) => aether_lspd::run_lspd(args).map(|()| ExitCode::SUCCESS),

        None => rt.block_on(run_default_command()).map_err(|e| e.clone()),
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run_default_command() -> Result<ExitCode, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    if should_run_onboarding(&cwd) {
        match run_new(NewArgs { path: PathBuf::from(".") }).await.map_err(|e| e.to_string())? {
            NewAgentOutcome::Applied => {
                wisp::run_tui("aether acp").await.map(|()| ExitCode::SUCCESS).map_err(|e| e.to_string())
            }
            NewAgentOutcome::Cancelled => Ok(ExitCode::SUCCESS),
        }
    } else {
        wisp::run_tui("aether acp").await.map(|()| ExitCode::SUCCESS).map_err(|e| e.to_string())
    }
}
