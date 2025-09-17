mod colors;
mod ui;

use aether::agent::{Agent, AgentMessage::*, UserMessage, agent};
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
use crossterm::{queue, style::Stylize};
use indicatif::ProgressBar;

#[derive(Debug)]
struct PartialToolCall {
    name: String,
    model_name: String,
    arguments: String,
    progress_bar: ProgressBar,
}
use inquire::Confirm;
use mcp_lexicon::AgentBuilderExt;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use tokio::fs;

#[derive(Parser)]
#[command(name = "wisp")]
#[command(about = "A TUI for the Aether AI assistant")]
struct Cli {
    #[arg(help = "The prompt to send to the AI assistant")]
    prompt: Vec<String>,

    #[arg(short = 's', long = "system", help = "The LLM's system prompt")]
    system: Option<String>,

    #[arg(
        short = 'u',
        long = "url",
        help = "HTTP endpoint URL for the LLM provider. Defaults to http://localhost:8080 (LLama.cpp server's default port)",
        default_value = "http://localhost:8080"
    )]
    url: String,

    #[arg(short = 'k', long = "api-key", help = "API key for the LLM provider")]
    api_key: Option<String>,

    #[arg(
        short = 'm',
        long = "model",
        help = "Model specification in format 'provider:model' or comma-separated for alloyed providers. Examples: 'anthropic:claude-3.5-sonnet', 'llamacpp', 'ollama:llama3.2,anthropic:claude-3-haiku'",
        default_value = "llamacpp"
    )]
    model: String,
}

#[derive(Debug, Clone)]
struct ModelSpec {
    provider: LlmProvider,
    model: String,
}

impl ModelSpec {
    fn parse(spec: &str) -> Result<Self, String> {
        if let Some((provider_str, model)) = spec.split_once(':') {
            let provider = Self::parse_provider(provider_str)?;
            Ok(ModelSpec {
                provider,
                model: model.to_string(),
            })
        } else {
            // For providers that don't require a model (like llamacpp), allow just the provider name
            match spec {
                "llamacpp" => Ok(ModelSpec {
                    provider: LlmProvider::LlamaCpp,
                    model: "".to_string(),
                }),
                _ => Err(format!(
                    "Invalid model spec '{}'. Expected format 'provider:model' or 'llamacpp'",
                    spec
                )),
            }
        }
    }

    fn parse_provider(provider_str: &str) -> Result<LlmProvider, String> {
        match provider_str {
            "anthropic" => Ok(LlmProvider::Anthropic),
            "openrouter" => Ok(LlmProvider::OpenRouter),
            "ollama" => Ok(LlmProvider::Ollama),
            "llamacpp" => Ok(LlmProvider::LlamaCpp),
            _ => Err(format!(
                "Unknown provider: {}. Supported providers: anthropic, openrouter, ollama, llamacpp",
                provider_str
            )),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // Create display string from model specs
    let init_display_name = if model_specs.len() == 1 {
        let spec = &model_specs[0];
        if spec.model.is_empty() {
            format!("{:?}", spec.provider)
        } else {
            format!("{:?} ({})", spec.provider, spec.model)
        }
    } else {
        let provider_names: Vec<String> = model_specs
            .iter()
            .map(|spec| {
                if spec.model.is_empty() {
                    format!("{:?}", spec.provider)
                } else {
                    format!("{:?} ({})", spec.provider, spec.model)
                }
            })
            .collect();
        format!("Alloyed [{}]", provider_names.join(", "))
    };

    let (mut agent, agents_status) = build_agent(provider, cli).await?;

    let (agents_loaded, agents_error) = match agents_status {
        AgentsStatus::Loaded => (true, None),
        AgentsStatus::NotFound => (false, None),
        AgentsStatus::Error(ref e) => (false, Some(e.as_str())),
    };
    ui::show_init_header(user_prompt, &init_display_name, agents_loaded, agents_error)?;
    let mut result_receiver = agent.send(UserMessage::text(user_prompt)).await;

    let mut active_tool_calls: HashMap<String, PartialToolCall> = HashMap::new();

    let mut message_started = false;

    while let Some(event) = result_receiver.recv().await {
        match event {
            Text {
                chunk,
                is_complete,
                model_name,
                ..
            } => {
                if is_complete {
                    print_styled!(stdout, "\n\n");
                    stdout.flush()?;
                    message_started = false;
                } else {
                    if let Some(filtered_chunk) = ui::filter_text_chunk(&chunk) {
                        if !message_started {
                            print_styled!(
                                stdout,
                                format!("{} ", "◈".with(colors::primary()).bold())
                            );
                            ui::show_model_info(&model_name)?;
                            message_started = true;
                        }

                        print_styled!(stdout, filtered_chunk.with(colors::text_primary()));
                        stdout.flush()?;
                    }
                }
            }

            ToolCall {
                tool_call_id,
                name,
                arguments,
                result,
                is_complete,
                model_name,
            } => {
                if is_complete {
                    if let Some(tool_call) = active_tool_calls.get(&tool_call_id) {
                        tool_call.progress_bar.finish_and_clear();
                        let args_to_show = if tool_call.arguments.is_empty() {
                            None
                        } else {
                            Some(tool_call.arguments.as_str())
                        };
                        ui::show_tool_completed(
                            &tool_call.name,
                            &tool_call.model_name,
                            args_to_show,
                            result.as_deref(),
                        )?;
                    }
                    active_tool_calls.remove(&tool_call_id);
                } else if !name.is_empty() {
                    // Tool starting - create spinner and initialize arguments
                    print_styled_line!(stdout, "");
                    stdout.flush()?;
                    let pb = ui::create_tool_spinner(&name, &model_name)?;
                    active_tool_calls.insert(
                        tool_call_id.clone(),
                        PartialToolCall {
                            name: name.clone(),
                            model_name: model_name.clone(),
                            arguments: String::new(),
                            progress_bar: pb,
                        },
                    );
                } else if let Some(args_chunk) = arguments {
                    // Tool argument chunk - accumulate arguments
                    if let Some(tool_call) = active_tool_calls.get_mut(&tool_call_id) {
                        tool_call.arguments.push_str(&args_chunk);
                    }
                }
            }

            Error { message } => {
                ui::show_error(&message)?;
            }

            Cancelled { message } => {
                ui::show_cancelled(&message)?;
            }

            ElicitationRequest {
                request,
                response_sender,
                ..
            } => {
                println!(
                    "\n{}",
                    "🤖 AI Request for Permission"
                        .with(colors::primary())
                        .bold()
                );
                println!("{}", request.message.with(colors::text_primary()));

                use aether::{CreateElicitationResult, ElicitationAction};

                let confirm_result = Confirm::new("Do you want to allow this action?")
                    .with_default(false)
                    .with_help_message("The AI is requesting permission to proceed")
                    .prompt();

                let result = match confirm_result {
                    Ok(true) => CreateElicitationResult {
                        action: ElicitationAction::Accept,
                        content: None,
                    },
                    Ok(false) => CreateElicitationResult {
                        action: ElicitationAction::Decline,
                        content: None,
                    },
                    Err(_) => CreateElicitationResult {
                        action: ElicitationAction::Cancel,
                        content: None,
                    },
                };

                let _ = response_sender.send(result);
                println!();
            }
        }
    }

    // Ensure any remaining output is flushed before showing completion
    stdout.flush()?;
    ui::show_completion()?;
    Ok(())
}

#[derive(Debug)]
enum AgentsStatus {
    Loaded,
    NotFound,
    Error(String),
}

async fn build_agent(
    provider: Box<dyn ModelProvider>,
    cli: &Cli,
) -> Result<(Agent<Box<dyn ModelProvider>>, AgentsStatus), Report> {
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

    let agent = agent(provider)
        .system_prompt(&combined_system)
        .coding_tools()
        .build()
        .await?;

    Ok((agent, agents_status))
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
            let provider = if spec.model.is_empty() {
                AnthropicProvider::default()?
            } else {
                AnthropicProvider::default_with_model(&spec.model)?
            };
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
