use serde::{Deserialize, Serialize};
use async_openai::types::{
    ChatCompletionStreamResponseDelta, FinishReason,
};

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
    pub index: u32,
    pub delta: ChatCompletionStreamResponseDelta,
    pub finish_reason: Option<FinishReason>,
    pub logprobs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}