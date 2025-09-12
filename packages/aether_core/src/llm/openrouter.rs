use async_openai::types::{
    ChatChoiceStream, ChatCompletionMessageToolCallChunk, ChatCompletionStreamResponseDelta,
    CreateChatCompletionRequest, CreateChatCompletionStreamResponse, FunctionCallStream,
};
use async_openai::{Client, config::OpenAIConfig};
use async_stream;
use color_eyre::Result;
use tokio_stream::{Stream, StreamExt};

use super::mappers::{map_messages, mapp_tools};
use super::openrouter_types::{
    CustomChatCompletionStreamChoice, CustomChatCompletionStreamResponse,
    CustomChatCompletionStreamResponseDelta, CustomFunctionCallDelta, CustomToolCallDelta,
    CustomUsage,
};
use super::provider::{ChatRequest, LlmProvider};
use super::streaming::process_completion_stream;
use crate::types::LlmMessage;

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
}

impl LlmProvider for OpenRouterProvider {
    fn complete_stream_chunks(
        &self,
        request: ChatRequest,
    ) -> impl Stream<Item = Result<LlmMessage>> + Send {
        let client = self.client.clone();
        let model = self.model.clone();

        async_stream::stream! {
            let messages = map_messages(request.messages);
            let tools = if request.tools.is_empty() {
                None
            } else {
                Some(mapp_tools(request.tools))
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
        }
    }
}

impl From<CustomChatCompletionStreamResponse> for CreateChatCompletionStreamResponse {
    fn from(custom: CustomChatCompletionStreamResponse) -> Self {
        CreateChatCompletionStreamResponse {
            id: custom.id,
            choices: custom
                .choices
                .into_iter()
                .map(|choice| choice.into())
                .collect(),
            created: custom.created as u32, // Convert u64 to u32
            model: custom.model,
            service_tier: None, // OpenRouter doesn't provide service tier information
            system_fingerprint: custom.system_fingerprint,
            object: custom.object,
            usage: custom.usage.map(|u| u.into()),
        }
    }
}

impl From<CustomChatCompletionStreamChoice> for ChatChoiceStream {
    fn from(choice: CustomChatCompletionStreamChoice) -> Self {
        ChatChoiceStream {
            index: choice.index as u32, // Convert i32 to u32
            delta: choice.delta.into(),
            finish_reason: choice.finish_reason,
            logprobs: None, // OpenRouter doesn't provide detailed logprobs in our custom type
        }
    }
}

impl From<CustomChatCompletionStreamResponseDelta> for ChatCompletionStreamResponseDelta {
    fn from(delta: CustomChatCompletionStreamResponseDelta) -> Self {
        ChatCompletionStreamResponseDelta {
            role: delta.role,
            content: delta.content,
            refusal: None, // OpenRouter doesn't support refusal field
            tool_calls: delta
                .tool_calls
                .map(|calls| calls.into_iter().map(|call| call.into()).collect()),
            function_call: None, // OpenRouter doesn't use legacy function_call
        }
    }
}

impl From<CustomToolCallDelta> for ChatCompletionMessageToolCallChunk {
    fn from(call: CustomToolCallDelta) -> Self {
        ChatCompletionMessageToolCallChunk {
            index: call.index as u32, // Convert i32 to u32
            id: call.id,
            r#type: call.tool_type.and_then(|t| {
                // Convert string to ChatCompletionToolType
                match t.as_str() {
                    "function" => Some(async_openai::types::ChatCompletionToolType::Function),
                    _ => None,
                }
            }),
            function: call.function.map(|f| f.into()),
        }
    }
}

impl From<CustomFunctionCallDelta> for FunctionCallStream {
    fn from(f: CustomFunctionCallDelta) -> Self {
        FunctionCallStream {
            name: f.name,
            arguments: f.arguments,
        }
    }
}

impl From<CustomUsage> for async_openai::types::CompletionUsage {
    fn from(u: CustomUsage) -> Self {
        async_openai::types::CompletionUsage {
            prompt_tokens: u.prompt_tokens as u32,
            completion_tokens: u.completion_tokens as u32,
            total_tokens: u.total_tokens as u32,
            completion_tokens_details: None,
            prompt_tokens_details: None,
        }
    }
}
