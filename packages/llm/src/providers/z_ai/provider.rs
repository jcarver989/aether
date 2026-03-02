use crate::provider::get_context_window;
use crate::providers::openai_compatible::{build_chat_request, create_custom_stream};
use crate::{
    Context, LlmError, LlmResponseStream, ProviderFactory, Result, StreamingModelProvider,
};
use async_openai::{Client, config::OpenAIConfig};

pub struct ZAiProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl ZAiProvider {
    pub fn new(api_key: String) -> Self {
        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://api.z.ai/api/coding/paas/v4".to_string());

        Self {
            client: Client::with_config(config),
            model: "GLM-4.6".to_string(),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
}

impl ProviderFactory for ZAiProvider {
    fn from_env() -> Result<Self> {
        let api_key = std::env::var("ZAI_API_KEY")
            .map_err(|_| LlmError::MissingApiKey("ZAI_API_KEY".to_string()))?;
        Ok(Self::new(api_key))
    }

    fn with_model(self, model: &str) -> Self {
        self.with_model(model)
    }
}

impl StreamingModelProvider for ZAiProvider {
    fn context_window(&self) -> Option<u32> {
        get_context_window("zai", &self.model)
    }

    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        let request = match build_chat_request(&self.model, context) {
            Ok(req) => req,
            Err(e) => return Box::pin(async_stream::stream! { yield Err(e); }),
        };
        create_custom_stream(&self.client, request)
    }

    fn display_name(&self) -> String {
        format!("Z.ai ({})", self.model)
    }
}
