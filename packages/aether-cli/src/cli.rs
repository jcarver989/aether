use clap::Parser;
use llm::parser::ModelProviderParser;
use std::io::{IsTerminal, Read as _, stdin};
use std::path::PathBuf;

use crate::error::CliError;

#[derive(Parser)]
#[command(name = "aether")]
#[command(about = "Headless AI coding agent")]
pub struct Cli {
    /// Prompt to send (reads stdin if omitted and stdin is not a TTY)
    prompt: Vec<String>,

    /// Model (e.g. "anthropic:claude-sonnet-4-5")
    #[arg(short, long)]
    model: String,

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
    pub output: OutputFormat,

    /// Verbose logging to stderr
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Pretty,
    Json,
}

impl Cli {
    pub fn resolve_prompt(&self) -> Result<String, CliError> {
        match self.prompt.as_slice() {
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

    pub fn resolve_model(&self) -> Result<Box<dyn llm::StreamingModelProvider>, CliError> {
        let (llm, _) = ModelProviderParser::default()
            .parse(&self.model)
            .map_err(|e| CliError::ModelError(e.to_string()))?;

        Ok(llm)
    }
}
