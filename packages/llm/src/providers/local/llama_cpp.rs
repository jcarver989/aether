use super::util::get_local_config;
use crate::providers::openai::OpenAiChatProvider;
use crate::{ProviderFactory, Result};
use async_openai::{Client, config::OpenAIConfig};

pub struct LlamaCppProvider {
    client: Client<OpenAIConfig>,
}

impl LlamaCppProvider {
    pub fn new(&self, base_url: &str) -> Self {
        Self {
            client: Client::with_config(get_local_config(base_url)),
        }
    }
}

impl Default for LlamaCppProvider {
    fn default() -> Self {
        Self {
            client: Client::with_config(get_local_config("http://localhost:8080/v1")),
        }
    }
}

impl ProviderFactory for LlamaCppProvider {
    fn from_env() -> Result<Self> {
        Ok(Self::default())
    }

    fn with_model(self, _model: &str) -> Self {
        // LlamaCpp doesn't support model selection - it serves a single model
        self
    }
}

impl OpenAiChatProvider for LlamaCppProvider {
    type Config = OpenAIConfig;

    fn client(&self) -> &Client<Self::Config> {
        &self.client
    }

    fn model(&self) -> &str {
        "" // llama.cpp server serves a single model on boot and does not allow swapping models
    }

    fn provider_name(&self) -> &str {
        "LlamaCpp"
    }
}
