use async_openai::types::chat::{
    ChatChoiceStream, ChatCompletionMessageToolCallChunk,
    ChatCompletionStreamResponseDelta as OpenAiDelta, CompletionUsage,
    CreateChatCompletionStreamResponse, FinishReason as OpenAiFinishReason, FunctionCallStream,
    FunctionType, Role,
};
use serde::{Deserialize, Serialize};

/// Unified custom types for OpenAI-compatible APIs that deviate slightly from the standard.
/// This handles quirks from providers like `OpenRouter`, Z.ai, and potentially others.
///
/// Common deviations handled:
/// - Missing 'object' field (z.ai)
/// - Negative token counts (openrouter)
/// - Additional finish reasons like 'error' (openrouter)
/// - Optional `system_fingerprint` and usage fields

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    FunctionCall,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionStreamResponse {
    pub id: String,
    pub choices: Vec<ChatCompletionStreamChoice>,
    pub created: u64,
    pub model: String,
    #[serde(default)]
    pub system_fingerprint: Option<String>,
    #[serde(default = "default_object")]
    pub object: String,
    #[serde(default)]
    pub usage: Option<Usage>,
}

fn default_object() -> String {
    "chat.completion.chunk".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionStreamChoice {
    pub index: i32,
    pub delta: ChatCompletionStreamResponseDelta,
    pub finish_reason: Option<FinishReason>,
    #[serde(default)]
    pub logprobs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionStreamResponseDelta {
    pub role: Option<Role>,
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: i32,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub tool_type: Option<String>,
    pub function: Option<FunctionCallDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

impl From<ChatCompletionStreamResponse> for CreateChatCompletionStreamResponse {
    #[allow(deprecated, clippy::cast_possible_truncation)]
    fn from(response: ChatCompletionStreamResponse) -> Self {
        CreateChatCompletionStreamResponse {
            id: response.id,
            choices: response.choices.into_iter().map(Into::into).collect(),
            created: response.created as u32,
            model: response.model,
            service_tier: None,
            system_fingerprint: response.system_fingerprint,
            object: response.object,
            usage: response.usage.map(Into::into),
        }
    }
}

impl From<FinishReason> for OpenAiFinishReason {
    fn from(reason: FinishReason) -> Self {
        match reason {
            FinishReason::Stop | FinishReason::Error => OpenAiFinishReason::Stop,
            FinishReason::Length => OpenAiFinishReason::Length,
            FinishReason::ToolCalls => OpenAiFinishReason::ToolCalls,
            FinishReason::ContentFilter => OpenAiFinishReason::ContentFilter,
            FinishReason::FunctionCall => OpenAiFinishReason::FunctionCall,
        }
    }
}

impl From<ChatCompletionStreamChoice> for ChatChoiceStream {
    #[allow(clippy::cast_sign_loss)]
    fn from(choice: ChatCompletionStreamChoice) -> Self {
        ChatChoiceStream {
            index: choice.index as u32,
            delta: choice.delta.into(),
            finish_reason: choice.finish_reason.map(std::convert::Into::into),
            logprobs: None,
        }
    }
}

impl From<ChatCompletionStreamResponseDelta> for OpenAiDelta {
    fn from(delta: ChatCompletionStreamResponseDelta) -> Self {
        OpenAiDelta {
            role: delta.role,
            content: delta.content,
            refusal: None,
            tool_calls: delta
                .tool_calls
                .map(|calls| calls.into_iter().map(Into::into).collect()),
            #[allow(deprecated)]
            function_call: None,
        }
    }
}

impl From<ToolCallDelta> for ChatCompletionMessageToolCallChunk {
    #[allow(clippy::cast_sign_loss)]
    fn from(call: ToolCallDelta) -> Self {
        ChatCompletionMessageToolCallChunk {
            index: call.index as u32,
            id: call.id,
            r#type: call
                .tool_type
                .filter(|t| t == "function")
                .map(|_| FunctionType::Function),
            function: call.function.map(Into::into),
        }
    }
}

impl From<FunctionCallDelta> for FunctionCallStream {
    fn from(f: FunctionCallDelta) -> Self {
        FunctionCallStream {
            name: f.name,
            arguments: f.arguments,
        }
    }
}

impl From<Usage> for CompletionUsage {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn from(u: Usage) -> Self {
        CompletionUsage {
            prompt_tokens: u.prompt_tokens.max(0) as u32,
            completion_tokens: u.completion_tokens.max(0) as u32,
            total_tokens: u.total_tokens.max(0) as u32,
            completion_tokens_details: None,
            prompt_tokens_details: None,
        }
    }
}
