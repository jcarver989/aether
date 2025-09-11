use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
    ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionTool, ChatCompletionToolType, FunctionCall, FunctionObject,
};

use crate::types::{ChatMessage, ToolDefinition};

impl From<ChatMessage> for Option<ChatCompletionRequestMessage> {
    fn from(msg: ChatMessage) -> Self {
        match msg {
            ChatMessage::System { content, .. } => Some(ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessage {
                    content: content.into(),
                    name: None,
                },
            )),
            ChatMessage::User { content, .. } => Some(ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessage {
                    content: content.into(),
                    name: None,
                },
            )),
            ChatMessage::Assistant {
                content,
                tool_calls,
                ..
            } => {
                let openai_tool_calls: Vec<_> = tool_calls
                    .into_iter()
                    .map(|call| ChatCompletionMessageToolCall {
                        id: call.id.clone(),
                        r#type: ChatCompletionToolType::Function,
                        function: FunctionCall {
                            name: call.name.clone(),
                            arguments: call.arguments.to_string(),
                        },
                    })
                    .collect();

                let tool_calls = if openai_tool_calls.is_empty() {
                    None
                } else {
                    Some(openai_tool_calls)
                };

                Some(ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessage {
                        content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                            content,
                        )),
                        name: None,
                        tool_calls,
                        audio: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                ))
            }
            ChatMessage::ToolCallResult {
                tool_call_id,
                content,
                ..
            } => Some(ChatCompletionRequestMessage::Tool(
                ChatCompletionRequestToolMessage {
                    content: ChatCompletionRequestToolMessageContent::Text(content),
                    tool_call_id,
                },
            )),

            ChatMessage::AssistantStreaming { .. } | ChatMessage::Error { .. } => None,
        }
    }
}

pub fn convert_messages(messages: Vec<ChatMessage>) -> Vec<ChatCompletionRequestMessage> {
    messages
        .into_iter()
        .filter_map(Into::into)
        .collect()
}

pub fn convert_tools(tools: Vec<ToolDefinition>) -> Vec<ChatCompletionTool> {
    tools
        .into_iter()
        .map(|tool| ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: tool.name,
                description: Some(tool.description),
                parameters: Some(serde_json::from_str(&tool.parameters).unwrap_or_default()),
                strict: Some(false),
            },
        })
        .collect()
}