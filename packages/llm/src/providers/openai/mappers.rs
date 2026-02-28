use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
    ChatCompletionRequestUserMessage, ChatCompletionTool, ChatCompletionTools, FunctionCall,
    FunctionObject,
};

use crate::{ChatMessage, LlmError, ToolDefinition};

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
                    .map(|call| {
                        ChatCompletionMessageToolCalls::Function(ChatCompletionMessageToolCall {
                            id: call.id,
                            function: FunctionCall {
                                name: call.name,
                                arguments: call.arguments,
                            },
                        })
                    })
                    .collect();

                let tool_calls = (!openai_tool_calls.is_empty()).then_some(openai_tool_calls);

                Some(ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessage {
                        content: Some(ChatCompletionRequestAssistantMessageContent::Text(content)),
                        name: None,
                        tool_calls,
                        audio: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                ))
            }
            ChatMessage::ToolCallResult(result) => {
                let (content, id) = match result {
                    Ok(r) => (r.result, r.id),
                    Err(e) => (e.error, e.id),
                };
                Some(ChatCompletionRequestMessage::Tool(
                    ChatCompletionRequestToolMessage {
                        content: ChatCompletionRequestToolMessageContent::Text(content),
                        tool_call_id: id,
                    },
                ))
            }
            ChatMessage::Summary { content, .. } => Some(ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessage {
                    content: format!("[Previous conversation handoff]\n\n{content}").into(),
                    name: None,
                },
            )),
            ChatMessage::Error { .. } => None,
        }
    }
}

impl From<&ChatMessage> for Option<ChatCompletionRequestMessage> {
    fn from(msg: &ChatMessage) -> Self {
        msg.clone().into()
    }
}

pub fn map_messages(messages: &[ChatMessage]) -> Vec<ChatCompletionRequestMessage> {
    messages.iter().filter_map(Into::into).collect()
}

pub fn map_tools(tools: &[ToolDefinition]) -> Result<Vec<ChatCompletionTools>, LlmError> {
    tools.iter().map(tool_definition_to_openai).collect()
}

fn tool_definition_to_openai(tool: &ToolDefinition) -> Result<ChatCompletionTools, LlmError> {
    let parameters =
        serde_json::from_str(&tool.parameters).map_err(|e| LlmError::ToolParameterParsing {
            tool_name: tool.name.clone(),
            error: e.to_string(),
        })?;

    Ok(ChatCompletionTools::Function(ChatCompletionTool {
        function: FunctionObject {
            name: tool.name.clone(),
            description: Some(tool.description.clone()),
            parameters: Some(parameters),
            strict: Some(false),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_tools_with_valid_json() {
        let tools = vec![ToolDefinition {
            name: "search".to_string(),
            description: "Search for things".to_string(),
            parameters: r#"{"type": "object", "properties": {"q": {"type": "string"}}}"#
                .to_string(),
            server: None,
        }];
        let result = map_tools(&tools);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn map_tools_with_invalid_json_returns_error() {
        let tools = vec![ToolDefinition {
            name: "broken_tool".to_string(),
            description: "A tool with bad params".to_string(),
            parameters: "not valid json{{{".to_string(),
            server: None,
        }];
        let result = map_tools(&tools);
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::ToolParameterParsing { tool_name, .. } => {
                assert_eq!(tool_name, "broken_tool");
            }
            other => panic!("Expected ToolParameterParsing, got: {other}"),
        }
    }
}
