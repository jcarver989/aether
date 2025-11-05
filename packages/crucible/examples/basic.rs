/// Basic example demonstrating how to run evals with Crucible
///
/// This example shows the minimal setup needed to run evaluations against
/// an agent. It uses the ModelProviderParser to support any provider.
///
/// # Usage
///
/// ```bash
/// cargo run --example basic -- --model openrouter:anthropic/claude-3-5-sonnet-20241022
/// cargo run --example basic -- --model ollama:llama3.3 --judge-model ollama:llama3.3
/// ```
use aether::llm::parser::ModelProviderParser;
use clap::Parser;
use crucible::{Crucible, EvalsConfig};
use mcp_lexicon::{CodingMcp, ServiceExt};

#[derive(Parser)]
#[command(name = "crucible-basic")]
#[command(about = "Basic Crucible evaluation example")]
struct Cli {
    #[arg(
        short = 'm',
        long = "model",
        help = "Model spec for the agent (e.g., 'openrouter:anthropic/claude-3-5-sonnet-20241022', 'ollama:llama3.3')",
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
    let llm = parser
        .parse(&cli.model)
        .map_err(|e| format!("Error parsing model spec '{}': {}", cli.model, e))?;

    // Parse the judge model (or use the same as agent model)
    let judge_model = cli.judge_model.as_ref().unwrap_or(&cli.model);
    let judge_llm = parser
        .parse(judge_model)
        .map_err(|e| format!("Error parsing judge model spec '{}': {}", judge_model, e))?;

    // Create configuration
    let config = EvalsConfig::new(llm, judge_llm);

    // Run evaluations
    // This will look for:
    // - ./examples/test-agent/AGENTS.md (optional system prompt)
    // - ./examples/test-agent/mcp.json (optional MCP server config)
    // - ./examples/test-agent/evals/* (eval directories)
    let summary = Crucible::new("./examples/test-agent".into())
        .with_server_factory("coding", Box::new(|_args| CodingMcp::new().into_dyn()))
        .run_evals(config)
        .await?;

    // Print results
    println!("\n{}", "=".repeat(50));
    println!("Evaluation Summary");
    println!("{}", "=".repeat(50));
    println!("Total: {}", summary.total_evals);
    println!("Passed: {}", summary.passed_evals);
    println!("Failed: {}", summary.failed_evals);
    println!(
        "Pass Rate: {:.1}%",
        (summary.passed_evals as f64 / summary.total_evals as f64) * 100.0
    );

    Ok(())
}
