mod app_state;
mod cli;
mod colors;
mod ui;
mod ui_event;

use aether::agent::{SpawnedAgent, UserMessage, agent};
use aether::llm::{
    ModelProvider,
    alloyed::AlloyedModelProvider,
    anthropic::AnthropicProvider,
    local::{llama_cpp::LlamaCppProvider, ollama::OllamaProvider},
    openrouter::OpenRouterProvider,
};
use aether::types::LlmProvider;
use clap::Parser;
use color_eyre::Report;
use indicatif::ProgressBar;

use mcp_lexicon::AgentBuilderExt;
use std::io::Write;
use std::path::Path;
use tokio::fs;
use tracing_subscriber;

use crate::app_state::AppState;
use crate::cli::{Cli, ModelSpec};

#[derive(Debug)]
struct PartialToolCall {
    name: String,
    model_name: String,
    arguments: String,
    progress_bar: ProgressBar,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing - set RUST_LOG env var to control log level
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    if cli.prompt.is_empty() {
        ui::show_usage("wisp")?;
        return Ok(());
    }

    let user_prompt = cli.prompt.join(" ");

    let model_specs = match parse_model_specs(&cli.model) {
        Ok(specs) => specs,
        Err(e) => {
            eprintln!("Error parsing model specification: {}", e);
            std::process::exit(1);
        }
    };

    let mut providers = Vec::new();
    for spec in &model_specs {
        match create_provider(spec) {
            Ok(provider) => providers.push(provider),
            Err(e) => {
                eprintln!("Error creating provider for {:?}: {}", spec.provider, e);
                std::process::exit(1);
            }
        }
    }

    // Use single provider or alloyed provider
    if providers.len() == 1 {
        let provider = providers.into_iter().next().unwrap();
        run_agent(provider, &model_specs, &cli, &user_prompt).await
    } else {
        let alloyed_provider = AlloyedModelProvider::new(providers);
        run_agent(Box::new(alloyed_provider), &model_specs, &cli, &user_prompt).await
    }
}

async fn run_agent(
    provider: Box<dyn ModelProvider>,
    model_specs: &[ModelSpec],
    cli: &Cli,
    user_prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = std::io::stdout();

    ui::show_wisp_logo()?;

    let init_display_name = ui::format_model_display_name(&model_specs);
    let (spawned_agent, agents_status) = spawn_agent(provider, cli).await?;

    let (agents_loaded, agents_error) = match agents_status {
        AgentsStatus::Loaded => (true, None),
        AgentsStatus::NotFound => (false, None),
        AgentsStatus::Error(ref e) => (false, Some(e.as_str())),
    };
    ui::show_init_header(user_prompt, &init_display_name, agents_loaded, agents_error)?;

    spawned_agent
        .tx
        .send(UserMessage::text(user_prompt))
        .await?;

    let mut app_state = AppState::new();
    let mut rx = spawned_agent.rx;

    while let Some(event) = rx.recv().await {
        let ui_events = app_state.update(event)?;
        ui::render_ui_events(ui_events)?;
    }

    // Ensure any remaining output is flushed before showing completion
    stdout.flush()?;
    ui::show_completion()?;

    // Clean up the spawned agent task
    spawned_agent.task_handle.abort();
    Ok(())
}

#[derive(Debug)]
enum AgentsStatus {
    Loaded,
    NotFound,
    Error(String),
}

async fn spawn_agent(
    provider: Box<dyn ModelProvider>,
    cli: &Cli,
) -> Result<(SpawnedAgent, AgentsStatus), Report> {
    let (system_prompt, agents_status) = match load_agents_file().await {
        Ok(Some(content)) => (Some(content), AgentsStatus::Loaded),
        Ok(None) => (None, AgentsStatus::NotFound),
        Err(e) => (None, AgentsStatus::Error(e.to_string())),
    };

    let system = cli.system.as_ref().map(|s| s.as_str()).unwrap_or("");
    let combined_system = if let Some(agents_prompt) = system_prompt {
        if system.is_empty() {
            agents_prompt
        } else {
            format!("{}\n\n{}", system, agents_prompt)
        }
    } else {
        system.to_string()
    };

    let spawned_agent = agent(provider)
        .system_prompt(&combined_system)
        .coding_tools()
        .spawn()
        .await?;

    Ok((spawned_agent, agents_status))
}

async fn load_agents_file() -> Result<Option<String>, std::io::Error> {
    let agents_file = Path::new("./AGENTS.md");

    if !agents_file.exists() || !agents_file.is_file() {
        return Ok(None);
    }

    fs::read_to_string(agents_file).await.map(Some)
}

fn create_provider(spec: &ModelSpec) -> Result<Box<dyn ModelProvider>, Box<dyn std::error::Error>> {
    use LlmProvider::*;
    match spec.provider {
        Anthropic => {
            let provider = AnthropicProvider::default()?.with_model(&spec.model);
            Ok(Box::new(provider))
        }
        OpenRouter => {
            let model = if spec.model.is_empty() {
                "anthropic/claude-3.5-sonnet"
            } else {
                &spec.model
            };
            let provider = OpenRouterProvider::default(model)?;
            Ok(Box::new(provider))
        }
        Ollama => {
            let model = if spec.model.is_empty() {
                "llama3.2"
            } else {
                &spec.model
            };
            let provider = OllamaProvider::default(model);
            Ok(Box::new(provider))
        }
        LlamaCpp => {
            let provider = LlamaCppProvider::default();
            Ok(Box::new(provider))
        }
    }
}

fn parse_model_specs(model_arg: &str) -> Result<Vec<ModelSpec>, String> {
    if model_arg.is_empty() || model_arg == "llamacpp:" || model_arg == "llamacpp" {
        return Ok(vec![ModelSpec {
            provider: LlmProvider::LlamaCpp,
            model: "".to_string(),
        }]);
    }

    model_arg
        .split(',')
        .map(|spec| ModelSpec::parse(spec.trim()))
        .collect()
}
