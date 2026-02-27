pub mod error;
pub mod run;

use error::CliError;
use llm::parser::ModelProviderParser;
use std::io::{IsTerminal, Read as _, stdin};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Clone)]
pub enum OutputFormat {
    Text,
    Pretty,
    Json,
}

pub struct RunConfig {
    pub prompt: String,
    pub model: Box<dyn llm::StreamingModelProvider>,
    pub cwd: PathBuf,
    pub mcp_config: Option<PathBuf>,
    pub system_prompt: Option<String>,
    pub output: OutputFormat,
    pub verbose: bool,
}

pub async fn run_headless(args: HeadlessArgs) -> Result<ExitCode, CliError> {
    let prompt = resolve_prompt(&args)?;
    let model = resolve_model(&args.model)?;

    let output = match args.output {
        CliOutputFormat::Text => OutputFormat::Text,
        CliOutputFormat::Pretty => OutputFormat::Pretty,
        CliOutputFormat::Json => OutputFormat::Json,
    };

    let config = RunConfig {
        prompt,
        model,
        cwd: args.cwd,
        mcp_config: args.mcp_config,
        system_prompt: args.system_prompt,
        output,
        verbose: args.verbose,
    };

    run::run(config).await
}

#[derive(Clone, clap::ValueEnum)]
pub enum CliOutputFormat {
    Text,
    Pretty,
    Json,
}

#[derive(clap::Args)]
pub struct HeadlessArgs {
    /// Prompt to send (reads stdin if omitted and stdin is not a TTY)
    pub prompt: Vec<String>,

    /// Model (e.g. "anthropic:claude-sonnet-4-5")
    #[arg(short, long)]
    pub model: String,

    /// Working directory
    #[arg(short = 'C', long = "cwd", default_value = ".")]
    pub cwd: PathBuf,

    /// Path to mcp.json (auto-detected if omitted)
    #[arg(long = "mcp-config")]
    pub mcp_config: Option<PathBuf>,

    /// Additional system prompt
    #[arg(long = "system-prompt")]
    pub system_prompt: Option<String>,

    /// Output format
    #[arg(long, default_value = "text")]
    pub output: CliOutputFormat,

    /// Verbose logging to stderr
    #[arg(short, long)]
    pub verbose: bool,
}

fn resolve_prompt(args: &HeadlessArgs) -> Result<String, CliError> {
    match args.prompt.as_slice() {
        args if !args.is_empty() => Ok(args.join(" ")),

        _ if !stdin().is_terminal() => {
            let mut buf = String::new();
            stdin()
                .read_to_string(&mut buf)
                .map_err(CliError::IoError)?;

            match buf.trim() {
                "" => Err(CliError::NoPrompt),
                s => Ok(s.to_string()),
            }
        }
        _ => Err(CliError::NoPrompt),
    }
}

fn resolve_model(model: &str) -> Result<Box<dyn llm::StreamingModelProvider>, CliError> {
    let (llm, _) = ModelProviderParser::default()
        .parse(model)
        .map_err(|e| CliError::ModelError(e.to_string()))?;

    Ok(llm)
}
