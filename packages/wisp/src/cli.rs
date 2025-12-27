use std::{fs, path::Path};

use aether::llm::StreamingModelProvider;
use aether::llm::parser::ModelProviderParser;
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

    #[arg(
        long = "log-dir",
        help = "Path to log file directory (default: /tmp/wisp-logs)"
    )]
    pub log_dir: Option<String>,
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
    ) -> Result<Box<dyn StreamingModelProvider>, Box<dyn std::error::Error>> {
        let parser = ModelProviderParser::default();
        Ok(parser.parse(&self.model)?)
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
