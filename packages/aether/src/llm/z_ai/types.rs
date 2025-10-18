use async_openai::types::{
    ChatChoiceStream, ChatCompletionMessageToolCallChunk, ChatCompletionStreamResponseDelta,
    CreateChatCompletionStreamResponse, FunctionCallStream, Role,
};
use serde::{Deserialize, Serialize};

/// Z.ai's API returns responses that are close to OpenAI format but with some differences:
/// - Missing the 'object' field (required by OpenAI)
/// - Optional system_fingerprint
/// These custom types handle the deserialization and convert to standard OpenAI types

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomFinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomChatCompletionStreamResponse {
    pub id: String,
    pub choices: Vec<CustomChatCompletionStreamChoice>,
    pub created: u64,
    pub model: String,
    #[serde(default)]
    pub system_fingerprint: Option<String>,
    #[serde(default = "default_object")]
    pub object: String,
}

fn default_object() -> String {
    "chat.completion.chunk".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomChatCompletionStreamChoice {
    pub index: i32,
    pub delta: CustomChatCompletionStreamResponseDelta,
    pub finish_reason: Option<CustomFinishReason>,
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

impl From<CustomChatCompletionStreamResponse> for CreateChatCompletionStreamResponse {
    fn from(custom: CustomChatCompletionStreamResponse) -> Self {
        CreateChatCompletionStreamResponse {
            id: custom.id,
            choices: custom
                .choices
                .into_iter()
                .map(|choice| choice.into())
                .collect(),
            created: custom.created as u32,
            model: custom.model,
            service_tier: None,
            system_fingerprint: custom.system_fingerprint,
            object: custom.object,
            usage: None,
        }
    }
}

impl From<CustomFinishReason> for async_openai::types::FinishReason {
    fn from(reason: CustomFinishReason) -> Self {
        match reason {
            CustomFinishReason::Stop => async_openai::types::FinishReason::Stop,
            CustomFinishReason::Length => async_openai::types::FinishReason::Length,
            CustomFinishReason::ToolCalls => async_openai::types::FinishReason::ToolCalls,
            CustomFinishReason::ContentFilter => async_openai::types::FinishReason::ContentFilter,
            CustomFinishReason::FunctionCall => async_openai::types::FinishReason::FunctionCall,
        }
    }
}

impl From<CustomChatCompletionStreamChoice> for ChatChoiceStream {
    fn from(choice: CustomChatCompletionStreamChoice) -> Self {
        ChatChoiceStream {
            index: choice.index as u32,
            delta: choice.delta.into(),
            finish_reason: choice.finish_reason.map(|r| r.into()),
            logprobs: None,
        }
    }
}

impl From<CustomChatCompletionStreamResponseDelta> for ChatCompletionStreamResponseDelta {
    fn from(delta: CustomChatCompletionStreamResponseDelta) -> Self {
        ChatCompletionStreamResponseDelta {
            role: delta.role,
            content: delta.content,
            refusal: None,
            tool_calls: delta
                .tool_calls
                .map(|calls| calls.into_iter().map(|call| call.into()).collect()),
            #[allow(deprecated)]
            function_call: None,
        }
    }
}

impl From<CustomToolCallDelta> for ChatCompletionMessageToolCallChunk {
    fn from(call: CustomToolCallDelta) -> Self {
        ChatCompletionMessageToolCallChunk {
            index: call.index as u32,
            id: call.id,
            r#type: call.tool_type.and_then(|t| match t.as_str() {
                "function" => Some(async_openai::types::ChatCompletionToolType::Function),
                _ => None,
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
