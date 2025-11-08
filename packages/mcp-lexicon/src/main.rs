use aether::llm::parser::ModelProviderParser;
use clap::{Parser, Subcommand};
use crucible::{AetherRunner, EvalRunner, EvalsConfig};
use mcp_lexicon::CodingMcp;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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

        /// Number of evaluations to run concurrently in each batch (default: run all concurrently)
        #[arg(long)]
        batch_size: Option<usize>,

        /// Delay between batches to prevent rate limiting (e.g., "2s", "1.5s", "500ms", "1m")
        #[arg(long, value_parser = parse_duration)]
        batch_delay: Option<Duration>,

        /// Serve the HTML report on localhost:3000 after evals complete
        #[arg(long, default_value = "false")]
        serve: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Evals {
            agent_model,
            judge_model,
            output_dir,
            batch_size,
            batch_delay,
            serve,
        } => {
            // Create output directory structure
            let output_dir_path = output_dir.unwrap_or_else(|| {
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                PathBuf::from(format!("crucible_output_{timestamp}"))
            });

            fs::create_dir_all(&output_dir_path)?;

            tracing::info!("Starting evaluations...");
            tracing::info!("Output directory: {}", output_dir_path.display());
            tracing::info!("Agent model: {}", agent_model);
            tracing::info!("Judge model: {}", judge_model);

            if let Some(batch_size) = batch_size {
                tracing::info!("Batch size: {}", batch_size);
            }
            if let Some(batch_delay) = batch_delay {
                tracing::info!("Batch delay: {:?}", batch_delay);
            }
            if serve {
                tracing::info!("Will serve report on http://localhost:3000");
            }

            let parser = ModelProviderParser::default();
            let agent_llm = parser
                .parse(&agent_model)
                .map_err(|e| format!("Failed to parse agent model '{agent_model}': {e}"))?;

            let judge_llm = parser
                .parse(&judge_model)
                .map_err(|e| format!("Failed to parse judge model '{judge_model}': {e}"))?;

            // Load evals programmatically
            let evals = mcp_lexicon::evals::all_evals()
                .map_err(|e| format!("Failed to load evals: {e}"))?;

            tracing::info!("Loaded {} evals", evals.len());

            let results_store = crucible::FileSystemStore::new(output_dir_path)
                .map_err(|e| format!("Failed to create results store: {e}"))?;

            let runner = AetherRunner::new(Arc::new(agent_llm))
                .with_mcp_server_factory("coding", Box::new(|_args| Box::new(CodingMcp::new())));

            let crucible = EvalRunner::new(runner, results_store);

            let mut config = EvalsConfig::new(judge_llm);

            // Apply batch configuration if provided
            if let Some(batch_size) = batch_size {
                config = config.with_batch_size(batch_size);
            }
            if let Some(batch_delay) = batch_delay {
                config = config.with_batch_delay(batch_delay);
            }
            if serve {
                config = config.with_serve(true);
            }

            let run_id = crucible
                .run_evals(evals, config)
                .await
                .map_err(|e| format!("Failed to run evals: {e}"))?;

            println!("\nRun ID: {}", run_id);

            Ok(())
        }
    }
}

/// Parse human-readable duration strings (e.g., "2s", "1.5s", "500ms", "1m")
fn parse_duration(s: &str) -> Result<Duration, String> {
    if let Some(seconds) = s.strip_suffix('s') {
        if let Some(ms) = seconds.strip_suffix("ms") {
            ms.parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|_| format!("Invalid milliseconds: {s}"))
        } else {
            seconds
                .parse::<f64>()
                .map(Duration::from_secs_f64)
                .map_err(|_| format!("Invalid seconds: {s}"))
        }
    } else if let Some(minutes) = s.strip_suffix('m') {
        minutes
            .parse::<u64>()
            .map(Duration::from_secs)
            .map(|secs| secs * 60)
            .map_err(|_| format!("Invalid minutes: {s}"))
    } else if let Some(hours) = s.strip_suffix('h') {
        hours
            .parse::<u64>()
            .map(Duration::from_secs)
            .map(|secs| secs * 3600)
            .map_err(|_| format!("Invalid hours: {s}"))
    } else {
        Err("Duration must end with 's', 'ms', 'm', or 'h' (e.g., '2s', '500ms', '1m')".to_string())
    }
}
