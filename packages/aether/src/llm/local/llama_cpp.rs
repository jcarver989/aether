use crate::llm::{local::util::get_local_config, openai::OpenAiChatProvider};
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

    pub fn default() -> Self {
        Self {
            client: Client::with_config(get_local_config("http://localhost:8080/v1")),
        }
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
}
