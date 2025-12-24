use async_openai::{Client, config::OpenAIConfig};

use crate::llm::openai_compatible::{build_chat_request, create_custom_stream};
use crate::llm::{
    Context, LlmError, LlmResponseStream, ProviderFactory, Result, StreamingModelProvider,
};

pub struct GeminiProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Result<Self> {
        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://generativelanguage.googleapis.com/v1beta/openai/");

        let client = Client::with_config(config);
        Ok(Self { client, model })
    }
}

impl ProviderFactory for GeminiProvider {
    fn from_env() -> Result<Self> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| LlmError::MissingApiKey("GEMINI_API_KEY".to_string()))?;

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://generativelanguage.googleapis.com/v1beta/openai/");

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

impl StreamingModelProvider for GeminiProvider {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        let request = build_chat_request(&self.model, context);
        create_custom_stream(&self.client, request)
    }

    fn display_name(&self) -> String {
        format!("Gemini ({})", self.model)
    }
}
