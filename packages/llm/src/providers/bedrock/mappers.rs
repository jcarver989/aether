use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message, SystemContentBlock, Tool, ToolConfiguration,
    ToolInputSchema, ToolResultBlock, ToolResultContentBlock, ToolResultStatus, ToolSpecification,
    ToolUseBlock,
};
use aws_smithy_types::{Document, Number};
use serde_json::Value;
use std::{collections::HashMap, fmt::Display, result};

use crate::{ChatMessage, LlmError, Result, ToolCallError, ToolCallResult, ToolDefinition};

fn bedrock_err(e: impl Display) -> LlmError {
    LlmError::Other(e.to_string())
}

pub fn map_messages(messages: &[ChatMessage]) -> Result<(Vec<SystemContentBlock>, Vec<Message>)> {
    let mut system_blocks = Vec::new();
    let mut bedrock_messages = Vec::new();
    for message in messages {
        match message {
            ChatMessage::System { content, .. } => {
                system_blocks.push(SystemContentBlock::Text(content.clone()));
            }

            ChatMessage::User { content, .. } => {
                bedrock_messages.push(build_user_message(content)?);
            }

            ChatMessage::Assistant {
                content,
                tool_calls,
                ..
            } => {
                bedrock_messages.push(map_assistant_message(content, tool_calls)?);
            }

            ChatMessage::ToolCallResult(result) => {
                bedrock_messages.push(map_tool_call_result(result)?);
            }

            ChatMessage::Error { message, .. } => {
                bedrock_messages.push(build_user_message(&format!("Error: {message}"))?);
            }

            ChatMessage::Summary { content, .. } => {
                bedrock_messages.push(build_user_message(&format!(
                    "[Previous conversation handoff]\n\n{content}"
                ))?);
            }
        }
    }

    Ok((system_blocks, bedrock_messages))
}

pub fn map_tools(tools: &[ToolDefinition]) -> Result<ToolConfiguration> {
    let bedrock_tools: Vec<Tool> =
        tools
            .iter()
            .map(|tool| {
                let schema_value: serde_json::Value = serde_json::from_str(&tool.parameters)
                    .map_err(|e| LlmError::ToolParameterParsing {
                        tool_name: tool.name.clone(),
                        error: e.to_string(),
                    })?;
                let spec = ToolSpecification::builder()
                    .name(&tool.name)
                    .description(&tool.description)
                    .input_schema(ToolInputSchema::Json(json_to_document(&schema_value)))
                    .build()
                    .map_err(bedrock_err)?;
                Ok(Tool::ToolSpec(spec))
            })
            .collect::<Result<_>>()?;

    ToolConfiguration::builder()
        .set_tools(Some(bedrock_tools))
        .build()
        .map_err(bedrock_err)
}

fn build_user_message(content: &str) -> Result<Message> {
    Message::builder()
        .role(ConversationRole::User)
        .content(ContentBlock::Text(content.to_string()))
        .build()
        .map_err(bedrock_err)
}

fn map_assistant_message(content: &str, tool_calls: &[crate::ToolCallRequest]) -> Result<Message> {
    if tool_calls.is_empty() {
        return Message::builder()
            .role(ConversationRole::Assistant)
            .content(ContentBlock::Text(content.to_string()))
            .build()
            .map_err(bedrock_err);
    }

    let mut builder = Message::builder().role(ConversationRole::Assistant);

    if !content.is_empty() {
        builder = builder.content(ContentBlock::Text(content.to_string()));
    }

    for tool_call in tool_calls {
        let input: serde_json::Value = serde_json::from_str(&tool_call.arguments)
            .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));

        let tool_use = ToolUseBlock::builder()
            .tool_use_id(&tool_call.id)
            .name(&tool_call.name)
            .input(json_to_document(&input))
            .build()
            .map_err(bedrock_err)?;

        builder = builder.content(ContentBlock::ToolUse(tool_use));
    }

    builder.build().map_err(bedrock_err)
}

fn map_tool_call_result(result: &result::Result<ToolCallResult, ToolCallError>) -> Result<Message> {
    let (id, content_text, status) = match result {
        Ok(tool_result) => (
            &tool_result.id,
            &tool_result.result,
            ToolResultStatus::Success,
        ),
        Err(tool_error) => (&tool_error.id, &tool_error.error, ToolResultStatus::Error),
    };

    let block = ToolResultBlock::builder()
        .tool_use_id(id)
        .content(ToolResultContentBlock::Text(content_text.clone()))
        .status(status)
        .build()
        .map_err(bedrock_err)?;

    Message::builder()
        .role(ConversationRole::User)
        .content(ContentBlock::ToolResult(block))
        .build()
        .map_err(bedrock_err)
}

fn json_to_document(value: &Value) -> Document {
    match value {
        Value::Null => Document::Null,
        Value::Bool(b) => Document::Bool(*b),
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Document::Number(Number::PosInt(u))
            } else if let Some(i) = n.as_i64() {
                Document::Number(Number::NegInt(i))
            } else if let Some(f) = n.as_f64() {
                Document::Number(Number::Float(f))
            } else {
                Document::Null
            }
        }
        Value::String(s) => Document::String(s.clone()),
        Value::Array(arr) => Document::Array(arr.iter().map(json_to_document).collect()),
        Value::Object(obj) => {
            let map: HashMap<String, Document> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_document(v)))
                .collect();
            Document::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolCallError, ToolCallRequest, ToolCallResult};
    use crate::types::IsoString;
    use aws_sdk_bedrockruntime::types::ContentBlock;

    #[test]
    fn test_map_simple_user_message() {
        let messages = vec![ChatMessage::User {
            content: "Hello".to_string(),
            timestamp: IsoString::now(),
        }];

        let (system, mapped) = map_messages(&messages).unwrap();
        assert!(system.is_empty());
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].role(), &ConversationRole::User);
        assert_eq!(mapped[0].content().len(), 1);
        assert!(mapped[0].content()[0].is_text());
    }

    #[test]
    fn test_map_system_message() {
        let messages = vec![
            ChatMessage::System {
                content: "You are helpful".to_string(),
                timestamp: IsoString::now(),
            },
            ChatMessage::User {
                content: "Hello".to_string(),
                timestamp: IsoString::now(),
            },
        ];

        let (system, mapped) = map_messages(&messages).unwrap();
        assert_eq!(system.len(), 1);
        assert!(system[0].is_text());
        assert_eq!(mapped.len(), 1);
    }

    #[test]
    fn test_map_assistant_with_tool_calls() {
        let messages = vec![ChatMessage::Assistant {
            content: "I'll help".to_string(),
            reasoning: Default::default(),
            timestamp: IsoString::now(),
            tool_calls: vec![ToolCallRequest {
                id: "call_1".to_string(),
                name: "search".to_string(),
                arguments: r#"{"query": "test"}"#.to_string(),
            }],
        }];

        let (_system, mapped) = map_messages(&messages).unwrap();
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].role(), &ConversationRole::Assistant);

        let content = mapped[0].content();
        assert_eq!(content.len(), 2);
        assert!(content[0].is_text());
        assert!(content[1].is_tool_use());
    }

    #[test]
    fn test_map_assistant_tool_calls_without_text() {
        let messages = vec![ChatMessage::Assistant {
            content: String::new(),
            reasoning: Default::default(),
            timestamp: IsoString::now(),
            tool_calls: vec![ToolCallRequest {
                id: "call_1".to_string(),
                name: "search".to_string(),
                arguments: r#"{"query": "test"}"#.to_string(),
            }],
        }];

        let (_system, mapped) = map_messages(&messages).unwrap();
        let content = mapped[0].content();
        // Empty text should not be included
        assert_eq!(content.len(), 1);
        assert!(content[0].is_tool_use());
    }

    #[test]
    fn test_map_tool_result_success() {
        let messages = vec![ChatMessage::ToolCallResult(Ok(ToolCallResult {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: "{}".to_string(),
            result: "found it".to_string(),
        }))];

        let (_system, mapped) = map_messages(&messages).unwrap();
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].role(), &ConversationRole::User);

        let content = mapped[0].content();
        assert_eq!(content.len(), 1);
        assert!(content[0].is_tool_result());
    }

    #[test]
    fn test_map_tool_result_error() {
        let messages = vec![ChatMessage::ToolCallResult(Err(ToolCallError {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: None,
            error: "not found".to_string(),
        }))];

        let (_system, mapped) = map_messages(&messages).unwrap();
        assert_eq!(mapped.len(), 1);

        let content = mapped[0].content();
        assert_eq!(content.len(), 1);
        assert!(content[0].is_tool_result());
    }

    #[test]
    fn test_map_error_message() {
        let messages = vec![ChatMessage::Error {
            message: "something broke".to_string(),
            timestamp: IsoString::now(),
        }];

        let (_system, mapped) = map_messages(&messages).unwrap();
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].role(), &ConversationRole::User);
        match &mapped[0].content()[0] {
            ContentBlock::Text(text) => assert!(text.contains("something broke")),
            other => panic!("Expected text, got {other:?}"),
        }
    }

    #[test]
    fn test_map_summary_message() {
        let messages = vec![ChatMessage::Summary {
            content: "we talked about stuff".to_string(),
            timestamp: IsoString::now(),
            messages_compacted: 10,
        }];

        let (_system, mapped) = map_messages(&messages).unwrap();
        assert_eq!(mapped.len(), 1);
        match &mapped[0].content()[0] {
            ContentBlock::Text(text) => {
                assert!(text.contains("[Previous conversation handoff]"));
                assert!(text.contains("we talked about stuff"));
            }
            other => panic!("Expected text, got {other:?}"),
        }
    }

    #[test]
    fn test_map_tools() {
        let tools = vec![ToolDefinition {
            name: "search".to_string(),
            description: "Search for information".to_string(),
            parameters: r#"{"type": "object", "properties": {"query": {"type": "string"}}}"#
                .to_string(),
            server: None,
        }];

        let config = map_tools(&tools).unwrap();
        assert_eq!(config.tools().len(), 1);
        match &config.tools()[0] {
            Tool::ToolSpec(spec) => {
                assert_eq!(spec.name(), "search");
                assert_eq!(spec.description(), Some("Search for information"));
            }
            other => panic!("Expected ToolSpec, got {other:?}"),
        }
    }

    #[test]
    fn test_map_tools_invalid_json() {
        let tools = vec![ToolDefinition {
            name: "broken".to_string(),
            description: "A broken tool".to_string(),
            parameters: "not valid json".to_string(),
            server: None,
        }];

        let result = map_tools(&tools);
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::ToolParameterParsing { tool_name, .. } => {
                assert_eq!(tool_name, "broken");
            }
            other => panic!("Expected ToolParameterParsing, got {other:?}"),
        }
    }

    #[test]
    fn test_json_to_document_primitives() {
        assert_eq!(json_to_document(&serde_json::Value::Null), Document::Null);
        assert_eq!(
            json_to_document(&serde_json::Value::Bool(true)),
            Document::Bool(true)
        );
        assert_eq!(
            json_to_document(&serde_json::Value::String("hello".to_string())),
            Document::String("hello".to_string())
        );
    }

    #[test]
    fn test_json_to_document_nested_object() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"type": "object", "properties": {"name": {"type": "string"}}}"#,
        )
        .unwrap();

        let doc = json_to_document(&json);
        match &doc {
            Document::Object(map) => {
                assert!(map.contains_key("type"));
                assert!(map.contains_key("properties"));
            }
            other => panic!("Expected Object, got {other:?}"),
        }
    }
}
