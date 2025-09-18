use aether::types::LlmProvider;
use clap::Parser;

#[derive(Parser)]
#[command(name = "wisp")]
#[command(about = "A TUI for the Aether AI assistant")]
pub struct Cli {
    #[arg(
        help = "The prompt to send to the AI assistant (optional - if not provided, starts interactive mode)"
    )]
    pub prompt: Vec<String>,

    #[arg(short = 's', long = "system", help = "The LLM's system prompt")]
    pub system: Option<String>,

    #[arg(
        short = 'u',
        long = "url",
        help = "HTTP endpoint URL for the LLM provider. Defaults to http://localhost:8080 (LLama.cpp server's default port)",
        default_value = "http://localhost:8080"
    )]
    pub url: String,

    #[arg(short = 'k', long = "api-key", help = "API key for the LLM provider")]
    pub api_key: Option<String>,

    #[arg(
        short = 'm',
        long = "model",
        help = "Model specification in format 'provider:model' or comma-separated for alloyed providers. Examples: 'anthropic:claude-3.5-sonnet', 'llamacpp', 'ollama:llama3.2,anthropic:claude-3-haiku'",
        default_value = "llamacpp"
    )]
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub provider: LlmProvider,
    pub model: String,
}

impl ModelSpec {
    pub fn parse(spec: &str) -> Result<Self, String> {
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
