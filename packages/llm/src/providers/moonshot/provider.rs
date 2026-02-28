use crate::providers::openai::mappers::map_tools;
use crate::providers::openai_compatible::streaming::create_custom_stream_generic;
use crate::{
    ChatMessage, Context, LlmError, LlmResponseStream, ProviderFactory, Result,
    StreamingModelProvider,
};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls, ChatCompletionTools,
        FunctionCall,
    },
};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct MoonshotChatRequest {
    model: String,
    messages: Vec<MoonshotChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ChatCompletionTools>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
enum MoonshotChatMessage {
    System {
        content: String,
    },
    User {
        content: String,
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

fn map_messages(messages: &[ChatMessage]) -> Vec<MoonshotChatMessage> {
    messages
        .iter()
        .filter_map(|message| match message {
            ChatMessage::System { content, .. } => Some(MoonshotChatMessage::System {
                content: content.clone(),
            }),
            ChatMessage::User { content, .. } => Some(MoonshotChatMessage::User {
                content: content.clone(),
            }),
            ChatMessage::Assistant {
                content,
                reasoning_content,
                tool_calls,
                ..
            } => {
                let openai_tool_calls: Vec<_> = tool_calls
                    .iter()
                    .map(|call| {
                        ChatCompletionMessageToolCalls::Function(ChatCompletionMessageToolCall {
                            id: call.id.clone(),
                            function: FunctionCall {
                                name: call.name.clone(),
                                arguments: call.arguments.clone(),
                            },
                        })
                    })
                    .collect();

                let tool_calls = (!openai_tool_calls.is_empty()).then_some(openai_tool_calls);

                Some(MoonshotChatMessage::Assistant {
                    content: content.clone(),
                    reasoning_content: reasoning_content.clone(),
                    tool_calls,
                })
            }
            ChatMessage::ToolCallResult(result) => {
                let (content, tool_call_id) = match result {
                    Ok(tool_result) => (tool_result.result.clone(), tool_result.id.clone()),
                    Err(tool_error) => (tool_error.error.clone(), tool_error.id.clone()),
                };

                Some(MoonshotChatMessage::Tool {
                    content,
                    tool_call_id,
                })
            }
            ChatMessage::Summary { content, .. } => Some(MoonshotChatMessage::User {
                content: format!("[Previous conversation handoff]\n\n{content}"),
            }),
            ChatMessage::Error { .. } => None,
        })
        .collect()
}

fn build_moonshot_request(
    model: &str,
    context: &Context,
) -> std::result::Result<MoonshotChatRequest, LlmError> {
    let tools = if context.tools().is_empty() {
        None
    } else {
        Some(map_tools(context.tools())?)
    };

    Ok(MoonshotChatRequest {
        model: model.to_string(),
        messages: map_messages(context.messages()),
        stream: Some(true),
        tools,
    })
}

pub struct MoonshotProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl MoonshotProvider {
    pub fn new(api_key: String) -> Self {
        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://api.moonshot.ai/v1".to_string());

        Self {
            client: Client::with_config(config),
            model: "moonshot-v1-8k".to_string(),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
}

impl ProviderFactory for MoonshotProvider {
    fn from_env() -> Result<Self> {
        let api_key = std::env::var("MOONSHOT_API_KEY")
            .map_err(|_| LlmError::MissingApiKey("MOONSHOT_API_KEY".to_string()))?;
        Ok(Self::new(api_key))
    }

    fn with_model(self, model: &str) -> Self {
        self.with_model(model)
    }
}

impl StreamingModelProvider for MoonshotProvider {
    fn stream_response(&self, context: &Context) -> LlmResponseStream {
        let request = match build_moonshot_request(&self.model, context) {
            Ok(req) => req,
            Err(e) => return Box::pin(async_stream::stream! { yield Err(e); }),
        };
        create_custom_stream_generic(&self.client, request)
    }

    fn display_name(&self) -> String {
        format!("Moonshot ({})", self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::IsoString;
    use crate::{ToolCallRequest, ToolDefinition};

    fn assistant_with_tool_call(reasoning_content: Option<&str>) -> ChatMessage {
        ChatMessage::Assistant {
            content: "".to_string(),
            reasoning_content: reasoning_content.map(ToString::to_string),
            timestamp: IsoString::now(),
            tool_calls: vec![ToolCallRequest {
                id: "call_1".to_string(),
                name: "test__tool".to_string(),
                arguments: "{\"path\":\"src/main.rs\"}".to_string(),
            }],
        }
    }

    fn context_with_assistant_message(message: ChatMessage) -> Context {
        Context::new(
            vec![
                ChatMessage::User {
                    content: "run a tool".to_string(),
                    timestamp: IsoString::now(),
                },
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
        let request = build_moonshot_request("kimi-k2.5", &context).unwrap();

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["messages"][1]["role"], "assistant");
        assert_eq!(json["messages"][1]["reasoning_content"], "trace chunk");
    }

    #[test]
    fn test_build_request_omits_reasoning_content_when_empty() {
        let context = context_with_assistant_message(assistant_with_tool_call(None));
        let request = build_moonshot_request("kimi-k2.5", &context).unwrap();

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["messages"][1]["role"], "assistant");
        assert!(json["messages"][1].get("reasoning_content").is_none());
    }
}
