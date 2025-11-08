/// Advanced example with batching, web server, and programmatic eval definition
///
/// This example demonstrates all the bells and whistles:
/// - Programmatic eval definition (no JSON files)
/// - Batch processing to avoid rate limits
/// - Interactive web server for viewing results
/// - Custom in-memory MCP server integration
///
/// # Usage
///
/// ```bash
/// cargo run --example advanced -- --model openrouter:anthropic/claude-3-5-sonnet-20241022
/// cargo run --example advanced -- --model ollama:llama3.3 --batch-size 2
/// ```
///
/// Then open http://localhost:3000 in your browser to view the interactive report.
/// Press Ctrl+C to stop the server.
use aether::llm::parser::ModelProviderParser;
use clap::Parser;
use crucible::{AetherRunner, BinaryMetric, Eval, EvalAssertion, EvalRunner, EvalsConfig, WorkingDirectory};
use mcp_lexicon::{CodingMcp, ServiceExt};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "crucible-advanced")]
#[command(about = "Advanced Crucible evaluation example with batching and web server")]
struct Cli {
    #[arg(
        short = 'm',
        long = "model",
        help = "Model spec for the agent",
        default_value = "openrouter:anthropic/claude-3-5-sonnet-20241022"
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    println!("Running evaluations with advanced configuration...");
    println!("The interactive report will be available at http://localhost:3000\n");

    let parser = ModelProviderParser::default();

    // Parse the agent model
    let llm = std::sync::Arc::new(
        parser
            .parse(&cli.model)
            .map_err(|e| format!("Error parsing model spec '{}': {}", cli.model, e))?,
    );

    // Parse the judge model (or use the same as agent model)
    let judge_model = cli.judge_model.as_ref().unwrap_or(&cli.model);
    let judge_llm = parser
        .parse(judge_model)
        .map_err(|e| format!("Error parsing judge model spec '{}': {}", judge_model, e))?;

    // Define evaluations programmatically
    let evals = vec![
        // Eval 1: Create a file
        Eval::new(
            "create_file",
            "Create a file called `greeting.txt` with the content \"Hello, World!\".",
            WorkingDirectory::empty()?,
            vec![
                EvalAssertion::file_exists("greeting.txt"),
                EvalAssertion::file_matches("greeting.txt", "Hello, World!"),
            ],
        ),
        // Eval 2: Run a bash command
        Eval::new(
            "simple_bash",
            "Please run the command `echo \"Hello from Crucible!\"` in the terminal and show me the output.",
            WorkingDirectory::empty()?,
            vec![
                EvalAssertion::tool_call_at_least("bash", 1),
                EvalAssertion::llm_judge(|_ctx| {
                    format!(
                        "Did the agent successfully run the echo command and display the output 'Hello from Crucible!'?\n\nRespond with JSON matching this schema:\n{}\n\nOnly return the JSON, no other text.",
                        BinaryMetric::json_schema()
                    )
                }),
            ],
        ),
    ];

    // Create agent runner with MCP server
    let runner = AetherRunner::new(llm)
        .with_mcp_server_factory("coding", Box::new(|_args| CodingMcp::new().into_dyn()));

    // Create configuration with all features enabled
    let config = EvalsConfig::new(judge_llm)
        .with_batch_size(cli.batch_size) // Run N evals concurrently
        .with_batch_delay(Duration::from_secs(cli.batch_delay)) // Wait between batches
        .with_serve(true); // Start web server

    // Create output directory and results store
    let output_dir = PathBuf::from("./eval-results");
    let results_store = crucible::FileSystemStore::new(output_dir)
        .map_err(|e| format!("Failed to create store: {}", e))?;

    let summary = EvalRunner::new(runner, results_store)
        .with_agent_prompt(
            "You are a helpful AI assistant with access to various tools for file operations, \
             shell commands, and more. Your goal is to complete the user's task efficiently and accurately."
        )
        .run_evals(evals, config)
        .await?;

    // Print results
    println!("\n{}", "=".repeat(50));
    println!("Evaluation Complete");
    println!("{}", "=".repeat(50));
    println!("Run ID: {}", summary);
    println!("\nView detailed results at http://localhost:3000/api/runs/{}", summary);

    Ok(())
}
