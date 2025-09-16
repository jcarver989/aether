use async_openai::types::{
    ChatChoiceStream, ChatCompletionMessageToolCallChunk, ChatCompletionStreamResponseDelta,
    CreateChatCompletionStreamResponse, FinishReason, FunctionCallStream, Role,
};
use serde::{Deserialize, Serialize};

/// OpenRouter can return negative token values,
/// so we had to impelemnt custon types to get around that

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomChatCompletionStreamResponse {
    pub id: String,
    pub choices: Vec<CustomChatCompletionStreamChoice>,
    pub created: u64,
    pub model: String,
    pub system_fingerprint: Option<String>,
    pub object: String,
    pub usage: Option<CustomUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomChatCompletionStreamChoice {
    pub index: i32,
    pub delta: CustomChatCompletionStreamResponseDelta,
    pub finish_reason: Option<FinishReason>,
    pub logprobs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomChatCompletionStreamResponseDelta {
    pub role: Option<Role>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<CustomToolCallDelta>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomToolCallDelta {
    pub index: i32,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub tool_type: Option<String>,
    pub function: Option<CustomFunctionCallDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFunctionCallDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
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
            #[allow(deprecated)]
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
