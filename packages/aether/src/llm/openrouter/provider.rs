use async_openai::types::CreateChatCompletionRequest;
use async_openai::{Client, config::OpenAIConfig};

use crate::llm::openai::mappers::{map_messages, map_tools};
use crate::llm::openai_compatible::create_custom_stream;
use crate::llm::{Context, LlmError, LlmResponseStream, Result, StreamingModelProvider, ProviderFactory};

pub struct OpenRouterProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: String, model: String) -> Result<Self> {
        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://openrouter.ai/api/v1");

        let client = Client::with_config(config);
        Ok(Self { client, model })
    }

    pub fn default(model: &str) -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| LlmError::MissingApiKey("OPENROUTER_API_KEY".to_string()))?;

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://openrouter.ai/api/v1");

        let client = Client::with_config(config);

        Ok(Self {
            client,
            model: model.to_string(),
        })
    }
}

impl ProviderFactory for OpenRouterProvider {
    fn from_env() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| LlmError::MissingApiKey("OPENROUTER_API_KEY".to_string()))?;

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://openrouter.ai/api/v1");

        let client = Client::with_config(config);

        Ok(Self {
            client,
            model: String::new(),
        })
    }

    fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
}

impl StreamingModelProvider for OpenRouterProvider {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        let messages = map_messages(context.messages());
        let tools = if context.tools().is_empty() {
            None
        } else {
            Some(map_tools(context.tools()))
        };

        let request = CreateChatCompletionRequest {
            model: self.model.clone(),
            messages,
            stream: Some(true),
            tools,
            ..Default::default()
        };

        create_custom_stream(&self.client, request)
    }

    fn display_name(&self) -> String {
        format!("OpenRouter ({})", self.model)
    }
}
