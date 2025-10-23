use aether::llm::parser::ModelProviderParser;
use clap::{Parser, Subcommand};
use crucible::{Crucible, EvalsConfig};
use mcp_lexicon::CodingMcp;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

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
            let output_dir_path = output_dir.clone().unwrap_or_else(|| {
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                PathBuf::from(format!("crucible_output_{}", timestamp))
            });

            fs::create_dir_all(&output_dir_path)?;

            // Copy HTML report templates immediately so users can open and refresh the report
            if let Err(e) = crucible::copy_report_templates(&output_dir_path) {
                eprintln!("Warning: Failed to copy report templates: {}", e);
            } else {
                println!(
                    "HTML report templates ready at {}/report/index.html",
                    output_dir_path.display()
                );
                println!("You can open this now and refresh to see traces as they appear");
            }

            // JSON traces for HTML report using tracing-appender for non-blocking writes
            let traces_file = tracing_appender::rolling::never(&output_dir_path, "traces.jsonl");
            let (non_blocking, guard) = tracing_appender::non_blocking(traces_file);

            let json_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_ansi(false);

            // Human-readable stdout
            let stdout_layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true);

            let env_filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

            tracing_subscriber::registry()
                .with(env_filter)
                .with(json_layer)
                .with(stdout_layer)
                .init();

            tracing::info!("Starting evaluations...");
            tracing::info!("Output directory: {}", output_dir_path.display());
            tracing::info!("JSON traces: {}/traces.jsonl", output_dir_path.display());
            tracing::info!("Agent model: {}", agent_model);
            tracing::info!("Judge model: {}", judge_model);

            if let Some(batch_size) = batch_size {
                tracing::info!("Batch size: {}", batch_size);
            }
            if let Some(batch_delay) = batch_delay {
                tracing::info!("Batch delay: {:?}", batch_delay);
            }

            let parser = ModelProviderParser::default();
            let agent_llm = parser
                .parse(&agent_model)
                .map_err(|e| format!("Failed to parse agent model '{}': {}", agent_model, e))?;

            let judge_llm = parser
                .parse(&judge_model)
                .map_err(|e| format!("Failed to parse judge model '{}': {}", judge_model, e))?;

            let crucible = Crucible::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests"))
                .with_server_factory("coding", Box::new(|_args| Box::new(CodingMcp::new())))
                .with_output_dir(output_dir_path.clone());

            let mut config = EvalsConfig::new(agent_llm, judge_llm);

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

            let summary = crucible
                .run_evals(config)
                .await
                .map_err(|e| format!("Failed to run evals: {}", e))?;

            summary.print();

            // Flush all traces before generating HTML report
            drop(guard);
            println!("Flushed all traces");

            // Generate HTML report after traces are flushed
            let traces_file = output_dir_path.join("traces.jsonl");
            if traces_file.exists() {
                match crucible::report::generate_html_report(
                    &output_dir_path,
                    &summary,
                    &traces_file,
                ) {
                    Ok(_) => {
                        println!(
                            "HTML report generated at {}/report/index.html",
                            output_dir_path.display()
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to generate HTML report: {}", e);
                    }
                }
            }

            if summary.failed_evals > 0 {
                std::process::exit(1);
            }

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
                .map_err(|_| format!("Invalid milliseconds: {}", s))
        } else {
            seconds
                .parse::<f64>()
                .map(Duration::from_secs_f64)
                .map_err(|_| format!("Invalid seconds: {}", s))
        }
    } else if let Some(minutes) = s.strip_suffix('m') {
        minutes
            .parse::<u64>()
            .map(Duration::from_secs)
            .map(|secs| secs * 60)
            .map_err(|_| format!("Invalid minutes: {}", s))
    } else if let Some(hours) = s.strip_suffix('h') {
        hours
            .parse::<u64>()
            .map(Duration::from_secs)
            .map(|secs| secs * 3600)
            .map_err(|_| format!("Invalid hours: {}", s))
    } else {
        Err("Duration must end with 's', 'ms', 'm', or 'h' (e.g., '2s', '500ms', '1m')".to_string())
    }
}
