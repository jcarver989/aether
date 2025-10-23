use crate::llm::{ProviderFactory, local::util::get_local_config, openai::OpenAiChatProvider};
use async_openai::{Client, config::OpenAIConfig};

pub struct OllamaProvider {
    model: String,
    client: Client<OpenAIConfig>,
}

impl OllamaProvider {
    pub fn new(&self, model: &str, base_url: &str) -> Self {
        Self {
            model: model.to_string(),
            client: Client::with_config(get_local_config(base_url)),
        }
    }

    pub fn default(model: &str) -> Self {
        Self {
            model: model.to_string(),
            client: Client::with_config(get_local_config("http://localhost:11434/v1")),
        }
    }
}

impl ProviderFactory for OllamaProvider {
    fn from_env() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            model: String::new(),
            client: Client::with_config(get_local_config("http://localhost:11434/v1")),
        })
    }

    fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
}

impl OpenAiChatProvider for OllamaProvider {
    type Config = OpenAIConfig;

    fn client(&self) -> &Client<Self::Config> {
        &self.client
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn provider_name(&self) -> &str {
        "Ollama"
    }
}
