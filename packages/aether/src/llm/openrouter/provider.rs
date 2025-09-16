use async_openai::types::CreateChatCompletionRequest;
use async_openai::{Client, config::OpenAIConfig};
use async_stream;
use color_eyre::Result;
use tokio_stream::StreamExt;

use crate::llm::openai::mappers::{map_messages, map_tools};
use crate::llm::openai::process_completion_stream;
use crate::llm::openrouter::CustomChatCompletionStreamResponse;
use crate::llm::{Context, LlmResponseStream, ModelProvider};

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
        let api_key = std::env::var("OPENROUTER_API_KEY").map_err(|_| {
            color_eyre::eyre::eyre!("OPENROUTER_API_KEY environment variable not set")
        })?;

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

impl ModelProvider for OpenRouterProvider {
    fn stream_response(&self, request: Context) -> LlmResponseStream {
        let client = self.client.clone();
        let model = self.model.clone();

        Box::pin(async_stream::stream! {
            let messages = map_messages(request.messages);
            let tools = if request.tools.is_empty() {
                None
            } else {
                Some(map_tools(request.tools))
            };

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
                    yield Err(color_eyre::eyre::eyre!("OpenRouter API request failed: {}", e));
                    return;
                }
            };

            // Convert custom responses to standard async_openai types and handle errors
            let standard_stream = stream.map(|result| {
                result
                    .map(|custom| custom.into())
                    .map_err(|e| color_eyre::eyre::eyre!("OpenRouter API error: {}", e))
            });

            let mut shared_stream = Box::pin(process_completion_stream(standard_stream));
            while let Some(result) = shared_stream.next().await {
                yield result;
            }
        })
    }
}
