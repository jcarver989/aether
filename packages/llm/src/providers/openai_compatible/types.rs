use async_openai::types::chat::{
    ChatChoiceStream, ChatCompletionMessageToolCall, ChatCompletionMessageToolCallChunk,
    ChatCompletionMessageToolCalls, ChatCompletionStreamOptions, ChatCompletionStreamResponseDelta as OpenAiDelta,
    ChatCompletionTools, CompletionUsage, CreateChatCompletionStreamResponse, FinishReason as OpenAiFinishReason,
    FunctionCall, FunctionCallStream, FunctionType, Role,
};
use serde::{Deserialize, Serialize};

use crate::{ChatMessage, ContentBlock};

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
    NetworkError,
    ModelContextWindowExceeded,
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

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum UserContent {
    Text(String),
    Parts(Vec<UserContentPart>),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrlContent },
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageUrlContent {
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum CompatibleChatMessage {
    System {
        content: String,
    },
    User {
        content: UserContent,
    },
    Assistant {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ChatCompletionMessageToolCalls>>,
    },
    Tool {
        content: String,
        tool_call_id: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct CompatibleChatRequest {
    pub model: String,
    pub messages: Vec<CompatibleChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatCompletionTools>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<ChatCompletionStreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<crate::ReasoningEffort>,
}

pub fn map_messages(messages: &[ChatMessage]) -> crate::Result<Vec<CompatibleChatMessage>> {
    let mut result = Vec::new();

    for message in messages {
        let mapped = match message {
            ChatMessage::System { content, .. } => Some(CompatibleChatMessage::System { content: content.clone() }),
            ChatMessage::User { content, .. } => {
                Some(CompatibleChatMessage::User { content: map_user_content(content)? })
            }
            ChatMessage::Assistant { content, reasoning, tool_calls, .. } => {
                let openai_tool_calls: Vec<_> = tool_calls
                    .iter()
                    .map(|call| {
                        ChatCompletionMessageToolCalls::Function(ChatCompletionMessageToolCall {
                            id: call.id.clone(),
                            function: FunctionCall { name: call.name.clone(), arguments: call.arguments.clone() },
                        })
                    })
                    .collect();

                let has_tool_calls = !openai_tool_calls.is_empty();
                let tool_calls = has_tool_calls.then_some(openai_tool_calls);

                let reasoning_content = if reasoning.summary_text.is_some() {
                    reasoning.summary_text.clone()
                } else if has_tool_calls {
                    Some(".".to_string())
                } else {
                    None
                };

                Some(CompatibleChatMessage::Assistant { content: content.clone(), reasoning_content, tool_calls })
            }
            ChatMessage::ToolCallResult(r) => {
                let (content, tool_call_id) = match r {
                    Ok(tool_result) => (tool_result.result.clone(), tool_result.id.clone()),
                    Err(tool_error) => (tool_error.error.clone(), tool_error.id.clone()),
                };

                Some(CompatibleChatMessage::Tool { content, tool_call_id })
            }
            ChatMessage::Summary { content, .. } => Some(CompatibleChatMessage::User {
                content: UserContent::Text(format!("[Previous conversation handoff]\n\n{content}")),
            }),
            ChatMessage::Error { .. } => None,
        };

        if let Some(msg) = mapped {
            result.push(msg);
        }
    }

    Ok(result)
}

fn map_user_content(parts: &[ContentBlock]) -> crate::Result<UserContent> {
    let has_non_text = parts.iter().any(|p| !matches!(p, ContentBlock::Text { .. }));

    if !has_non_text {
        return Ok(UserContent::Text(ContentBlock::join_text(parts)));
    }

    let mut items = Vec::with_capacity(parts.len());
    for p in parts {
        match p {
            ContentBlock::Text { text } => items.push(UserContentPart::Text { text: text.clone() }),
            ContentBlock::Image { .. } => {
                items.push(UserContentPart::ImageUrl { image_url: ImageUrlContent { url: p.as_data_uri().unwrap() } })
            }
            ContentBlock::Audio { .. } => {
                return Err(crate::LlmError::UnsupportedContent("This provider does not support audio input".into()));
            }
        }
    }

    Ok(UserContent::Parts(items))
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
pub struct PromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    #[serde(default)]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
}

impl From<ChatCompletionStreamResponse> for CreateChatCompletionStreamResponse {
    #[allow(deprecated)]
    fn from(response: ChatCompletionStreamResponse) -> Self {
        CreateChatCompletionStreamResponse {
            id: response.id,
            choices: response.choices.into_iter().map(Into::into).collect(),
            created: u32::try_from(response.created).unwrap_or(0),
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
            FinishReason::Stop | FinishReason::Error | FinishReason::NetworkError => OpenAiFinishReason::Stop,
            FinishReason::Length | FinishReason::ModelContextWindowExceeded => OpenAiFinishReason::Length,
            FinishReason::ToolCalls => OpenAiFinishReason::ToolCalls,
            FinishReason::ContentFilter => OpenAiFinishReason::ContentFilter,
            FinishReason::FunctionCall => OpenAiFinishReason::FunctionCall,
        }
    }
}

impl From<ChatCompletionStreamChoice> for ChatChoiceStream {
    fn from(choice: ChatCompletionStreamChoice) -> Self {
        ChatChoiceStream {
            index: u32::try_from(choice.index).unwrap_or(0),
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
            tool_calls: delta.tool_calls.map(|calls| calls.into_iter().map(Into::into).collect()),
            #[allow(deprecated)]
            function_call: None,
        }
    }
}

impl From<ToolCallDelta> for ChatCompletionMessageToolCallChunk {
    fn from(call: ToolCallDelta) -> Self {
        ChatCompletionMessageToolCallChunk {
            index: u32::try_from(call.index).unwrap_or(0),
            id: call.id,
            r#type: call.tool_type.filter(|t| t == "function").map(|_| FunctionType::Function),
            function: call.function.map(Into::into),
        }
    }
}

impl From<FunctionCallDelta> for FunctionCallStream {
    fn from(f: FunctionCallDelta) -> Self {
        FunctionCallStream { name: f.name, arguments: f.arguments }
    }
}

impl From<Usage> for CompletionUsage {
    fn from(u: Usage) -> Self {
        CompletionUsage {
            prompt_tokens: u32::try_from(u.prompt_tokens.max(0)).unwrap_or(0),
            completion_tokens: u32::try_from(u.completion_tokens.max(0)).unwrap_or(0),
            total_tokens: u32::try_from(u.total_tokens.max(0)).unwrap_or(0),
            completion_tokens_details: None,
            prompt_tokens_details: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::IsoString;
    use crate::{ToolCallRequest, ToolDefinition};

    fn assistant_with_tool_call(reasoning_content: Option<&str>) -> ChatMessage {
        ChatMessage::Assistant {
            content: String::new(),
            reasoning: crate::AssistantReasoning {
                summary_text: reasoning_content.map(ToString::to_string),
                encrypted_content: None,
            },
            timestamp: IsoString::now(),
            tool_calls: vec![ToolCallRequest {
                id: "call_1".to_string(),
                name: "test__tool".to_string(),
                arguments: "{\"path\":\"src/main.rs\"}".to_string(),
            }],
        }
    }

    fn context_with_assistant_message(message: ChatMessage) -> crate::Context {
        crate::Context::new(
            vec![
                ChatMessage::User { content: vec![ContentBlock::text("run a tool")], timestamp: IsoString::now() },
                message,
            ],
            vec![ToolDefinition {
                name: "test__tool".to_string(),
                description: "test".to_string(),
                parameters: "{\"type\":\"object\"}".to_string(),
                server: None,
            }],
        )
    }

    #[test]
    fn test_build_request_includes_reasoning_content_on_assistant_tool_message() {
        let context = context_with_assistant_message(assistant_with_tool_call(Some("trace chunk")));
        let request = crate::providers::openai_compatible::build_chat_request("test-model", &context).unwrap();

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["messages"][1]["role"], "assistant");
        assert_eq!(json["messages"][1]["reasoning_content"], "trace chunk");
    }

    #[test]
    fn test_build_request_includes_stream_options_with_usage() {
        let context = crate::Context::new(
            vec![ChatMessage::User { content: vec![ContentBlock::text("hello")], timestamp: IsoString::now() }],
            vec![],
        );
        let request = crate::providers::openai_compatible::build_chat_request("test-model", &context).unwrap();

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["stream_options"]["include_usage"], true);
    }

    #[test]
    fn test_build_request_sends_empty_reasoning_content_on_tool_call_when_none() {
        let context = context_with_assistant_message(assistant_with_tool_call(None));
        let request = crate::providers::openai_compatible::build_chat_request("test-model", &context).unwrap();

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["messages"][1]["role"], "assistant");
        assert_eq!(json["messages"][1]["reasoning_content"], ".");
    }

    #[test]
    fn test_user_message_text_only_serializes_as_string() {
        let content = map_user_content(&[ContentBlock::text("Hello")]).unwrap();
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json, "Hello");
    }

    #[test]
    fn test_user_message_with_image_serializes_as_array() {
        let content = map_user_content(&[
            ContentBlock::text("Look:"),
            ContentBlock::Image { data: "aW1n".to_string(), mime_type: "image/png".to_string() },
        ])
        .unwrap();
        let json = serde_json::to_value(&content).unwrap();
        let parts = json.as_array().expect("Expected array");
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "Look:");
        assert_eq!(parts[1]["type"], "image_url");
        assert!(parts[1]["image_url"]["url"].as_str().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_user_message_audio_only_errors() {
        let result = map_user_content(&[ContentBlock::Audio {
            data: "YXVkaW8=".to_string(),
            mime_type: "audio/wav".to_string(),
        }]);
        assert!(matches!(result, Err(crate::LlmError::UnsupportedContent(_))));
    }

    #[test]
    fn test_user_message_audio_with_text_errors() {
        let result = map_user_content(&[
            ContentBlock::text("Listen:"),
            ContentBlock::Audio { data: "YXVkaW8=".to_string(), mime_type: "audio/wav".to_string() },
        ]);
        assert!(matches!(result, Err(crate::LlmError::UnsupportedContent(_))));
    }
}
