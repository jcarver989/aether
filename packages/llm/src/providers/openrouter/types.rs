use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionStreamOptions, ChatCompletionToolChoiceOption,
    ChatCompletionTools, CreateChatCompletionRequest, ResponseFormat, StopConfiguration,
};
use serde::{Deserialize, Serialize};

/// OpenRouter-specific usage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterUsage {
    #[serde(rename = "include")]
    pub include: bool,
}

/// Custom request type for `OpenRouter` that includes the usage parameter
///
/// `OpenRouter` requires a specific `usage` parameter in the request body to enable
/// token usage tracking. See: <https://openrouter.ai/docs/use-cases/usage-accounting>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterChatRequest {
    pub model: String,
    pub messages: Vec<ChatCompletionRequestMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatCompletionTools>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ChatCompletionToolChoiceOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<ChatCompletionStreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenRouterUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopConfiguration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<crate::ReasoningEffort>,
}

impl From<CreateChatCompletionRequest> for OpenRouterChatRequest {
    fn from(request: CreateChatCompletionRequest) -> Self {
        Self {
            model: request.model,
            messages: request.messages,
            stream: request.stream,
            tools: request.tools,
            tool_choice: request.tool_choice,
            temperature: request.temperature,
            top_p: request.top_p,
            max_completion_tokens: request.max_completion_tokens,
            stream_options: Some(ChatCompletionStreamOptions {
                include_usage: Some(true),
                include_obfuscation: None,
            }),
            usage: Some(OpenRouterUsage { include: true }),
            presence_penalty: request.presence_penalty,
            frequency_penalty: request.frequency_penalty,
            stop: request.stop,
            response_format: request.response_format,
            reasoning_effort: None,
        }
    }
}
