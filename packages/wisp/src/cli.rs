use std::{fs, path::Path};

use aether::{
    llm::{
        ModelProvider,
        alloyed::AlloyedModelProvider,
        anthropic::AnthropicProvider,
        local::{llama_cpp::LlamaCppProvider, ollama::OllamaProvider},
        openrouter::OpenRouterProvider,
    },
    types::LlmProvider,
};
use clap::Parser;

#[derive(Parser, Clone)]
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

impl Cli {
    pub fn load_system_prompt(&self) -> Option<SystemPrompt> {
        self.system
            .as_ref()
            .map(|s| SystemPrompt::CliArg(s.to_string()))
            .or_else(|| {
                let agents_file = Path::new("./AGENTS.md");
                if agents_file.exists() && agents_file.is_file() {
                    fs::read_to_string(agents_file)
                        .ok()
                        .map(SystemPrompt::AgentsMd)
                } else {
                    None
                }
            })
    }

    pub fn load_model_provider(
        &self,
    ) -> Result<(Box<dyn ModelProvider>, Vec<ModelSpec>), Box<dyn std::error::Error>> {
        use LlmProvider::*;
        let model_result: Result<Vec<ModelSpec>, String> = self
            .model
            .split(',')
            .map(|spec| ModelSpec::from_str(spec.trim()))
            .collect();

        let model_specs = model_result?;

        let mut providers = Vec::new();
        for spec in &model_specs {
            let provider: Box<dyn ModelProvider> = match spec.provider {
                Anthropic => {
                    let provider = AnthropicProvider::default()?.with_model(&spec.model);
                    Box::new(provider)
                }
                OpenRouter => {
                    let provider = OpenRouterProvider::default(&spec.model)?;
                    Box::new(provider)
                }
                Ollama => {
                    let provider = OllamaProvider::default(&spec.model);
                    Box::new(provider)
                }
                LlamaCpp => {
                    let provider = LlamaCppProvider::default();
                    Box::new(provider)
                }
            };

            providers.push(provider);
        }

        let provider: Box<dyn ModelProvider> = if providers.len() == 1 {
            providers.into_iter().next().unwrap()
        } else {
            Box::new(AlloyedModelProvider::new(providers))
        };

        Ok((provider, model_specs))
    }
}

#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub provider: LlmProvider,
    pub model: String,
}

impl ModelSpec {
    fn from_str(spec: &str) -> Result<Self, String> {
        if spec == "llamacpp" {
            return Ok(ModelSpec {
                provider: LlmProvider::LlamaCpp,
                model: "".to_string(),
            });
        }

        match spec.split_once(":") {
            Some((provider_str, model)) => Ok(ModelSpec {
                provider: LlmProvider::from_str(provider_str)?,
                model: model.to_string(),
            }),

            None => Err(format!(
                "Invalid model spec '{}'. Expected format 'provider:model' or 'llamacpp'",
                spec
            )),
        }
    }
}

pub enum SystemPrompt {
    AgentsMd(String),
    CliArg(String),
}

impl SystemPrompt {
    pub fn as_str(&self) -> &str {
        match self {
            SystemPrompt::AgentsMd(prompt) => prompt.as_ref(),
            SystemPrompt::CliArg(prompt) => prompt.as_ref(),
        }
    }
}
