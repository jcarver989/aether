use super::types::OpenRouterChatRequest;
use crate::providers::openai_compatible::{
    build_chat_request, streaming::create_custom_stream_generic,
};
use crate::{
    Context, LlmError, LlmResponseStream, ProviderFactory, Result, StreamingModelProvider,
};
use async_openai::{Client, config::OpenAIConfig};

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
    fn from_env() -> Result<Self> {
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
        // Build base request and convert to OpenRouter-specific format
        // The From trait automatically adds usage tracking parameters
        // See: https://openrouter.ai/docs/use-cases/usage-accounting
        let request: OpenRouterChatRequest = build_chat_request(&self.model, context).into();
        create_custom_stream_generic(&self.client, request)
    }

    fn display_name(&self) -> String {
        format!("OpenRouter ({})", self.model)
    }
}
