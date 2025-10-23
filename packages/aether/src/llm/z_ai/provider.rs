use crate::llm::openai::mappers::{map_messages, map_tools};
use crate::llm::openai_compatible::create_custom_stream;
use crate::llm::{Context, LlmError, LlmResponseStream, ProviderFactory, StreamingModelProvider};
use async_openai::{Client, config::OpenAIConfig, types::CreateChatCompletionRequest};

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
    fn from_env() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let api_key = std::env::var("ZAI_API_KEY")
            .map_err(|_| LlmError::MissingApiKey("ZAI_API_KEY".to_string()))?;
        Ok(Self::new(api_key))
    }

    fn with_model(self, model: &str) -> Self {
        self.with_model(model)
    }
}

impl StreamingModelProvider for ZAiProvider {
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
        format!("Z.ai ({})", self.model)
    }
}
