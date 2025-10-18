use crate::llm::{LlmError, Result, Context, LlmResponseStream, StreamingModelProvider};
use crate::llm::openai::mappers::{map_messages, map_tools};
use crate::llm::openai::process_completion_stream;
use crate::llm::z_ai::CustomChatCompletionStreamResponse;
use async_openai::{Client, config::OpenAIConfig, types::CreateChatCompletionRequest};
use async_stream;
use tokio_stream::StreamExt;

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

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ZAI_API_KEY")
            .map_err(|_| LlmError::MissingApiKey("ZAI_API_KEY".to_string()))?;
        Ok(Self::new(api_key))
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
}

impl StreamingModelProvider for ZAiProvider {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        let client = self.client.clone();
        let model = self.model.clone();
        let messages = map_messages(context.messages());
        let tools = if context.tools().is_empty() {
            None
        } else {
            Some(map_tools(context.tools()))
        };

        Box::pin(async_stream::stream! {
            let req = CreateChatCompletionRequest {
                model: model.clone(),
                messages,
                stream: Some(true),
                tools,
                ..Default::default()
            };

            let stream = match client
                .chat()
                .create_stream_byot::<CreateChatCompletionRequest, CustomChatCompletionStreamResponse>(req)
                .await {
                Ok(stream) => stream,
                Err(e) => {
                    yield Err(LlmError::ApiRequest(e.to_string()));
                    return;
                }
            };

            // Convert custom responses to standard async_openai types and handle errors
            let standard_stream = stream.map(|result| {
                result
                    .map(|custom| custom.into())
                    .map_err(|e| LlmError::ApiError(e.to_string()))
            });

            let mut shared_stream = Box::pin(process_completion_stream(standard_stream));
            while let Some(result) = shared_stream.next().await {
                yield result;
            }
        })
    }

    fn display_name(&self) -> String {
        format!("Z.ai ({})", self.model)
    }
}
