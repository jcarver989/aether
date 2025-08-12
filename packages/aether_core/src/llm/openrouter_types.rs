use async_openai::types::{FinishReason, Role};
use serde::{Deserialize, Serialize};

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
