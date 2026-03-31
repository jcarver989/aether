use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestMessageContentPartAudio, ChatCompletionRequestMessageContentPartImage,
    ChatCompletionRequestMessageContentPartText, ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
    ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
    ChatCompletionRequestUserMessageContentPart, ChatCompletionTool, ChatCompletionTools, FunctionCall, FunctionObject,
    ImageUrl, InputAudio, InputAudioFormat,
};

use crate::{ChatMessage, ContentBlock, LlmError, Result, ToolDefinition};

fn map_message(msg: ChatMessage) -> Result<Option<ChatCompletionRequestMessage>> {
    match msg {
        ChatMessage::System { content, .. } => {
            Ok(Some(ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: content.into(),
                name: None,
            })))
        }
        ChatMessage::User { content, .. } => {
            Ok(Some(ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: map_user_content(content)?,
                name: None,
            })))
        }
        ChatMessage::Assistant { content, tool_calls, .. } => {
            let openai_tool_calls: Vec<_> = tool_calls
                .into_iter()
                .map(|call| {
                    ChatCompletionMessageToolCalls::Function(ChatCompletionMessageToolCall {
                        id: call.id,
                        function: FunctionCall { name: call.name, arguments: call.arguments },
                    })
                })
                .collect();

            let tool_calls = (!openai_tool_calls.is_empty()).then_some(openai_tool_calls);

            Ok(Some(ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content: Some(ChatCompletionRequestAssistantMessageContent::Text(content)),
                name: None,
                tool_calls,
                audio: None,
                refusal: None,
                #[allow(deprecated)]
                function_call: None,
            })))
        }
        ChatMessage::ToolCallResult(result) => {
            let (content, id) = match result {
                Ok(r) => (r.result, r.id),
                Err(e) => (e.error, e.id),
            };
            Ok(Some(ChatCompletionRequestMessage::Tool(ChatCompletionRequestToolMessage {
                content: ChatCompletionRequestToolMessageContent::Text(content),
                tool_call_id: id,
            })))
        }
        ChatMessage::Summary { content, .. } => {
            Ok(Some(ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: format!("[Previous conversation handoff]\n\n{content}").into(),
                name: None,
            })))
        }
        ChatMessage::Error { .. } => Ok(None),
    }
}

pub fn map_messages(messages: &[ChatMessage]) -> Result<Vec<ChatCompletionRequestMessage>> {
    messages
        .iter()
        .cloned()
        .map(map_message)
        .filter_map(|result| match result {
            Ok(Some(message)) => Some(Ok(message)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

pub fn map_tools(tools: &[ToolDefinition]) -> Result<Vec<ChatCompletionTools>> {
    tools.iter().map(tool_definition_to_openai).collect()
}

fn map_user_content(parts: Vec<ContentBlock>) -> Result<ChatCompletionRequestUserMessageContent> {
    let openai_parts: Vec<ChatCompletionRequestUserMessageContentPart> =
        parts.into_iter().map(map_user_content_part).collect::<Result<_>>()?;

    Ok(ChatCompletionRequestUserMessageContent::Array(openai_parts))
}

fn map_user_content_part(part: ContentBlock) -> Result<ChatCompletionRequestUserMessageContentPart> {
    match part {
        ContentBlock::Text { text } => {
            Ok(ChatCompletionRequestUserMessageContentPart::Text(ChatCompletionRequestMessageContentPartText { text }))
        }
        ContentBlock::Image { data, mime_type } => {
            Ok(ChatCompletionRequestUserMessageContentPart::ImageUrl(ChatCompletionRequestMessageContentPartImage {
                image_url: ImageUrl { url: format!("data:{mime_type};base64,{data}"), detail: None },
            }))
        }
        ContentBlock::Audio { data, mime_type } => {
            let format = map_audio_format(&mime_type)?;
            Ok(ChatCompletionRequestUserMessageContentPart::InputAudio(ChatCompletionRequestMessageContentPartAudio {
                input_audio: InputAudio { data, format },
            }))
        }
    }
}

fn map_audio_format(mime_type: &str) -> Result<InputAudioFormat> {
    match mime_type {
        "audio/wav" => Ok(InputAudioFormat::Wav),
        "audio/mpeg" | "audio/mp3" => Ok(InputAudioFormat::Mp3),
        _ => Err(LlmError::UnsupportedContent(format!(
            "OpenAI chat completions does not support {mime_type} audio input"
        ))),
    }
}

fn tool_definition_to_openai(tool: &ToolDefinition) -> Result<ChatCompletionTools> {
    let parameters = serde_json::from_str(&tool.parameters)
        .map_err(|e| LlmError::ToolParameterParsing { tool_name: tool.name.clone(), error: e.to_string() })?;

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
    use crate::types::IsoString;

    #[test]
    fn map_user_content_text_only() {
        let parts = vec![ContentBlock::text("Hello")];
        let result = map_user_content(parts).unwrap();
        match result {
            ChatCompletionRequestUserMessageContent::Array(parts) => {
                assert_eq!(parts.len(), 1);
                assert!(matches!(
                    &parts[0],
                    ChatCompletionRequestUserMessageContentPart::Text(t) if t.text == "Hello"
                ));
            }
            other => panic!("Expected Array, got {other:?}"),
        }
    }

    #[test]
    fn map_user_content_with_image_produces_array() {
        let parts = vec![
            ContentBlock::text("Look at this:"),
            ContentBlock::Image { data: "aW1hZ2VkYXRh".to_string(), mime_type: "image/png".to_string() },
        ];
        let result = map_user_content(parts).unwrap();
        match result {
            ChatCompletionRequestUserMessageContent::Array(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(&parts[0], ChatCompletionRequestUserMessageContentPart::Text(_)));
                match &parts[1] {
                    ChatCompletionRequestUserMessageContentPart::ImageUrl(img) => {
                        assert!(img.image_url.url.starts_with("data:image/png;base64,"));
                    }
                    other => panic!("Expected ImageUrl, got {other:?}"),
                }
            }
            other => panic!("Expected Array, got {other:?}"),
        }
    }

    #[test]
    fn map_user_content_with_audio_produces_array() {
        let parts = vec![
            ContentBlock::text("Listen:"),
            ContentBlock::Audio { data: "YXVkaW9kYXRh".to_string(), mime_type: "audio/wav".to_string() },
        ];
        let result = map_user_content(parts).unwrap();
        match result {
            ChatCompletionRequestUserMessageContent::Array(parts) => {
                assert_eq!(parts.len(), 2);
                match &parts[1] {
                    ChatCompletionRequestUserMessageContentPart::InputAudio(aud) => {
                        assert_eq!(aud.input_audio.format, InputAudioFormat::Wav);
                    }
                    other => panic!("Expected InputAudio, got {other:?}"),
                }
            }
            other => panic!("Expected Array, got {other:?}"),
        }
    }

    #[test]
    fn map_user_content_with_mpeg_audio_produces_mp3_format() {
        let parts = vec![
            ContentBlock::text("Listen:"),
            ContentBlock::Audio { data: "YXVkaW9kYXRh".to_string(), mime_type: "audio/mpeg".to_string() },
        ];
        let result = map_user_content(parts).unwrap();
        match result {
            ChatCompletionRequestUserMessageContent::Array(parts) => match &parts[1] {
                ChatCompletionRequestUserMessageContentPart::InputAudio(aud) => {
                    assert_eq!(aud.input_audio.format, InputAudioFormat::Mp3);
                }
                other => panic!("Expected InputAudio, got {other:?}"),
            },
            other => panic!("Expected Array, got {other:?}"),
        }
    }

    #[test]
    fn map_user_content_with_mp3_audio_produces_mp3_format() {
        let parts = vec![
            ContentBlock::text("Listen:"),
            ContentBlock::Audio { data: "YXVkaW9kYXRh".to_string(), mime_type: "audio/mp3".to_string() },
        ];
        let result = map_user_content(parts).unwrap();
        match result {
            ChatCompletionRequestUserMessageContent::Array(parts) => match &parts[1] {
                ChatCompletionRequestUserMessageContentPart::InputAudio(aud) => {
                    assert_eq!(aud.input_audio.format, InputAudioFormat::Mp3);
                }
                other => panic!("Expected InputAudio, got {other:?}"),
            },
            other => panic!("Expected Array, got {other:?}"),
        }
    }

    #[test]
    fn map_user_content_with_ogg_returns_unsupported_content() {
        let parts = vec![
            ContentBlock::text("Listen:"),
            ContentBlock::Audio { data: "YXVkaW9kYXRh".to_string(), mime_type: "audio/ogg".to_string() },
        ];

        assert!(matches!(map_user_content(parts), Err(LlmError::UnsupportedContent(_))));
    }

    #[test]
    fn map_text_only_user_message_unchanged() {
        let messages =
            vec![ChatMessage::User { content: vec![ContentBlock::text("Hello")], timestamp: IsoString::now() }];
        let result = map_messages(&messages).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn map_messages_with_ogg_audio_returns_unsupported_content() {
        let messages = vec![ChatMessage::User {
            content: vec![
                ContentBlock::text("Listen:"),
                ContentBlock::Audio { data: "YXVkaW9kYXRh".to_string(), mime_type: "audio/ogg".to_string() },
            ],
            timestamp: IsoString::now(),
        }];

        assert!(matches!(map_messages(&messages), Err(LlmError::UnsupportedContent(_))));
    }

    #[test]
    fn map_tools_with_valid_json() {
        let tools = vec![ToolDefinition {
            name: "search".to_string(),
            description: "Search for things".to_string(),
            parameters: r#"{"type": "object", "properties": {"q": {"type": "string"}}}"#.to_string(),
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
