use aether::llm::parser::ModelProviderParser;
use clap::{Parser, Subcommand};
use color_eyre::Result;
use color_eyre::eyre::eyre;
use crucible::{Crucible, EvalsConfig};
use mcp_lexicon::CodingMcp;
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(name = "mcp-lexicon")]
#[command(about = "MCP Lexicon - A coding assistant evaluation tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run evaluations using the Crucible framework
    Evals {
        /// Model spec for agent LLM (e.g., 'anthropic:claude-3-5-sonnet', 'openrouter:anthropic/claude-3-5-sonnet')
        #[arg(long, env = "AGENT_MODEL", default_value = "llamacpp")]
        agent_model: String,

        /// Model spec for judge LLM (e.g., 'anthropic:claude-3-5-sonnet', 'openrouter:anthropic/claude-3-5-sonnet')
        #[arg(long, env = "JUDGE_MODEL", default_value = "llamacpp")]
        judge_model: String,

        /// Output directory for logs and results
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Evals {
            agent_model,
            judge_model,
            output_dir,
        } => {
            tracing::info!("Starting evaluations...");
            tracing::info!("Agent model: {}", agent_model);
            tracing::info!("Judge model: {}", judge_model);

            let parser = ModelProviderParser::default();
            let agent_llm = parser
                .parse(&agent_model)
                .map_err(|e| eyre!("Failed to parse agent model '{}': {}", agent_model, e))?;

            let judge_llm = parser
                .parse(&judge_model)
                .map_err(|e| eyre!("Failed to parse judge model '{}': {}", judge_model, e))?;

            let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let evals_base = package_root.join("tests");

            tracing::info!("Looking for evals in: {:?}", evals_base);

            let mut crucible = Crucible::new(evals_base)
                .with_server_factory("coding", Box::new(|| Box::new(CodingMcp::new())));

            if let Some(output) = output_dir {
                crucible = crucible.with_output_dir(output);
            }

            // Run evaluations
            let config = EvalsConfig::new(agent_llm, judge_llm);
            let summary = crucible
                .run_evals(config)
                .await
                .map_err(|e| color_eyre::eyre::eyre!("Failed to run evals: {}", e))?;

            summary.print();

            if summary.failed_evals > 0 {
                std::process::exit(1);
            }

            Ok(())
        }
    }
}
