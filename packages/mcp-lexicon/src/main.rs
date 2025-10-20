use aether::llm::parser::ModelProviderParser;
use clap::{Parser, Subcommand};
use color_eyre::Result;
use color_eyre::eyre::eyre;
use crucible::{Crucible, EvalsConfig};
use mcp_lexicon::CodingMcp;
use std::fs;
use std::path::PathBuf;
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
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Evals {
            agent_model,
            judge_model,
            output_dir,
        } => {
            // Create output directory structure
            let output_dir_path = output_dir
                .as_ref()
                .map(|p| p.clone())
                .unwrap_or_else(|| {
                    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                    PathBuf::from(format!("crucible_output_{}", timestamp))
                });

            fs::create_dir_all(&output_dir_path)?;

            // Copy HTML report templates immediately so users can open and refresh the report
            if let Err(e) = crucible::copy_report_templates(&output_dir_path) {
                eprintln!("Warning: Failed to copy report templates: {}", e);
            } else {
                println!("HTML report templates ready at {}/report/index.html", output_dir_path.display());
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

            let parser = ModelProviderParser::default();
            let agent_llm = parser
                .parse(&agent_model)
                .map_err(|e| eyre!("Failed to parse agent model '{}': {}", agent_model, e))?;

            let judge_llm = parser
                .parse(&judge_model)
                .map_err(|e| eyre!("Failed to parse judge model '{}': {}", judge_model, e))?;

            let crucible = Crucible::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests"))
                .with_server_factory("coding", Box::new(|| Box::new(CodingMcp::new())))
                .with_output_dir(output_dir_path.clone());

            let config = EvalsConfig::new(agent_llm, judge_llm);
            let summary = crucible
                .run_evals(config)
                .await
                .map_err(|e| color_eyre::eyre::eyre!("Failed to run evals: {}", e))?;

            summary.print();

            // Flush all traces before generating HTML report
            drop(guard);
            println!("Flushed all traces");

            // Generate HTML report after traces are flushed
            let traces_file = output_dir_path.join("traces.jsonl");
            if traces_file.exists() {
                match crucible::report::generate_html_report(&output_dir_path, &summary, &traces_file) {
                    Ok(_) => {
                        println!("HTML report generated at {}/report/index.html", output_dir_path.display());
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
