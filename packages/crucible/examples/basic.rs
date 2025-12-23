/// Basic example demonstrating how to run evals with Crucible using programmatic API
///
/// This example shows how to define evaluations directly in Rust code instead of
/// loading them from JSON files. It uses the ModelProviderParser to support any provider.
///
/// # Usage
///
/// ```bash
/// cargo run --example basic -- --model openrouter:anthropic/claude-sonnet-4-5-20250929
/// cargo run --example basic -- --model ollama:llama3.3 --judge-model ollama:llama3.3
/// ```
use aether::llm::parser::ModelProviderParser;
use clap::Parser;
use crucible::{
    AetherRunner, BinaryMetric, Eval, EvalAssertion, EvalRunner, EvalsConfig, WorkingDirectory,
};
use futures::FutureExt;
use mcp_lexicon::{CodingMcp, ServiceExt};

#[derive(Parser)]
#[command(name = "crucible-basic")]
#[command(about = "Basic Crucible evaluation example")]
struct Cli {
    #[arg(
        short = 'm',
        long = "model",
        help = "Model spec for the agent (e.g., 'openrouter:anthropic/claude-sonnet-4-5-20250929', 'ollama:llama3.3')",
        default_value = "zai:GLM-4.6"
    )]
    model: String,

    #[arg(
        short = 'j',
        long = "judge-model",
        help = "Model spec for the judge LLM (defaults to same as --model)"
    )]
    judge_model: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

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
    let runner = AetherRunner::new(llm).with_mcp_server_factory(
        "coding",
        Box::new(|_args| async move { CodingMcp::new().into_dyn() }.boxed()),
    );

    // Create configuration
    let config = EvalsConfig::new(judge_llm);

    // Create output directory and results store
    let output_dir = std::env::current_dir()?.join("crucible_output_basic");
    let results_store = crucible::FileSystemStore::new(output_dir)
        .map_err(|e| format!("Failed to create store: {}", e))?;

    // Run evaluations with system prompt
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

    Ok(())
}
