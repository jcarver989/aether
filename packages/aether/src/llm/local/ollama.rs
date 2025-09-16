use crate::llm::{local::util::get_local_config, openai::OpenAiChatProvider};
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

impl OpenAiChatProvider for OllamaProvider {
    type Config = OpenAIConfig;

    fn client(&self) -> &Client<Self::Config> {
        &self.client
    }

    fn model(&self) -> &str {
        &self.model
    }
}
