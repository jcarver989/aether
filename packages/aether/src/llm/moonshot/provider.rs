use crate::llm::openai_compatible::{build_chat_request, create_custom_stream};
use crate::llm::{
    Context, LlmError, LlmResponseStream, ProviderFactory, Result, StreamingModelProvider,
};
use async_openai::{Client, config::OpenAIConfig};

pub struct MoonshotProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl MoonshotProvider {
    pub fn new(api_key: String) -> Self {
        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://api.moonshot.ai/v1".to_string());

        Self {
            client: Client::with_config(config),
            model: "moonshot-v1-8k".to_string(),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
}

impl ProviderFactory for MoonshotProvider {
    fn from_env() -> Result<Self> {
        let api_key = std::env::var("MOONSHOT_API_KEY")
            .map_err(|_| LlmError::MissingApiKey("MOONSHOT_API_KEY".to_string()))?;
        Ok(Self::new(api_key))
    }

    fn with_model(self, model: &str) -> Self {
        self.with_model(model)
    }
}

impl StreamingModelProvider for MoonshotProvider {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        let request = build_chat_request(&self.model, context);
        create_custom_stream(&self.client, request)
    }

    fn display_name(&self) -> String {
        format!("Moonshot ({})", self.model)
    }
}
