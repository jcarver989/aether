use async_openai::{Client, config::OpenAIConfig};
use async_stream;
use tokio_stream::StreamExt;

use crate::llm::openai::process_completion_stream;
use crate::llm::openai_compatible::{build_chat_request, types::ChatCompletionStreamResponse};
use crate::llm::openrouter::types::OpenRouterChatRequest;
use crate::llm::{
    Context, LlmError, LlmResponseStream, ProviderFactory, Result, StreamingModelProvider,
};

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
        let request: OpenRouterChatRequest = build_chat_request(&self.model, context).into();
        let client = self.client.clone();

        Box::pin(async_stream::stream! {
            let stream = match client
                .chat()
                .create_stream_byot::<OpenRouterChatRequest, ChatCompletionStreamResponse>(request)
                .await {
                Ok(stream) => stream,
                Err(e) => {
                    yield Err(LlmError::ApiRequest(e.to_string()));
                    return;
                }
            };

            let mapped_stream = stream.map(|result| {
                result
                    .map(|response| response.into())
                    .map_err(|e| LlmError::ApiError(e.to_string()))
            });

            for await item in process_completion_stream(mapped_stream) {
                yield item;
            }
        })
    }

    fn display_name(&self) -> String {
        format!("OpenRouter ({})", self.model)
    }
}
