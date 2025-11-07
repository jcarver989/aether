/// Planning Agent Evaluation Runner
///
/// This runner executes evaluations for the planning agent using the Crucible framework.
/// It supports batching, web server for viewing results, and custom MCP servers.
///
/// # Usage
///
/// ```bash
/// cargo run -p planning-agent -- --model openrouter:anthropic/claude-3-5-sonnet-20241022
/// cargo run -p planning-agent -- --model ollama:llama3.3 --batch-size 2
/// ```
///
/// Then open http://localhost:3000 in your browser to view the interactive report.
/// Press Ctrl+C to stop the server.
use aether::{agent::Prompt, llm::parser::ModelProviderParser};
use clap::Parser;
use crucible::{EvalRunner, EvalsConfig};
use mcp_lexicon::{CodingMcp, ServiceExt};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "planning-agent")]
#[command(about = "Planning agent evaluation runner with Crucible")]
struct Cli {
    #[arg(
        short = 'm',
        long = "model",
        help = "Model spec for the agent",
        default_value = "zai:GLM-4.6"
    )]
    model: String,

    #[arg(
        short = 'j',
        long = "judge-model",
        help = "Model spec for the judge LLM (defaults to same as --model)"
    )]
    judge_model: Option<String>,

    #[arg(
        short = 'b',
        long = "batch-size",
        help = "Number of evals to run concurrently",
        default_value = "3"
    )]
    batch_size: usize,

    #[arg(
        short = 'd',
        long = "batch-delay",
        help = "Delay in seconds between batches",
        default_value = "2"
    )]
    batch_delay: u64,

    #[arg(
        short = 'e',
        long = "evals-dir",
        help = "Directory containing the evaluations",
        default_value = "./tests"
    )]
    evals_dir: String,

    #[arg(
        short = 'o',
        long = "output-dir",
        help = "Directory for evaluation results",
        default_value = "./eval-results"
    )]
    output_dir: String,

    #[arg(long = "no-serve", help = "Disable the web server for viewing results")]
    no_serve: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Running planning agent evaluations...");

    let parser = ModelProviderParser::default();

    let llm = parser
        .parse(&cli.model)
        .map_err(|e| format!("Error parsing model spec '{}': {}", cli.model, e))?;

    let judge_model = cli.judge_model.as_ref().unwrap_or(&cli.model);
    let judge_llm = parser
        .parse(judge_model)
        .map_err(|e| format!("Error parsing judge model spec '{}': {}", judge_model, e))?;

    // Load evals programmatically
    let evals =
        planning_agent::evals::all_evals().map_err(|e| format!("Failed to load evals: {e}"))?;

    tracing::info!("Loaded {} evals", evals.len());

    let config = EvalsConfig::new(llm, judge_llm)
        .with_batch_size(cli.batch_size)
        .with_batch_delay(Duration::from_secs(cli.batch_delay))
        .with_serve(!cli.no_serve);

    let results_store = crucible::FileSystemStore::new(cli.output_dir.into())
        .map_err(|e| format!("Failed to create results store: {e}"))?;

    let run_id = EvalRunner::new(results_store)
        .with_mcp_server_factory("coding", Box::new(|_args| CodingMcp::new().into_dyn()))
        .with_mcp_json("mcp.json")
        .with_agent_prompt(Prompt::file("./tests/AGENTS.md", false).build()?)
        .run_evals(evals, config)
        .await?;

    tracing::info!("\n{}", "=".repeat(50));
    tracing::info!("Evaluation Complete");
    tracing::info!("{}", "=".repeat(50));
    tracing::info!("Run ID: {}", run_id);

    if !cli.no_serve {
        tracing::info!("\nView detailed results at http://localhost:3000/api/runs/{}", run_id);
        tracing::info!("Press Ctrl+C to stop the server.");
    }

    Ok(())
}
