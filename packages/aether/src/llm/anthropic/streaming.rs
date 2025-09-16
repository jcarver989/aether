use super::types::{AnthropicStreamEvent, ContentBlockDeltaData, ContentBlockStartData};
use crate::types::{LlmResponse, ToolCallRequest};
use async_stream;
use color_eyre::Result;
use futures::Stream;
use std::collections::HashMap;
use tokio_stream::StreamExt;
use tracing::{debug, warn};

pub fn process_anthropic_stream<T: Stream<Item = Result<String>> + Send + Sync + Unpin>(
    stream: T,
) -> impl Stream<Item = Result<LlmResponse>> + Send {
    async_stream::stream! {
        let message_id = uuid::Uuid::new_v4().to_string();
        yield Ok(LlmResponse::Start { message_id: message_id.clone() });

        let mut active_tool_calls: HashMap<u32, (String, String, String)> = HashMap::new();
        let mut text_content = String::new();

        let mut stream = Box::pin(stream);
        while let Some(result) = stream.next().await {
            match result {
                Ok(line) => {
                    if line.trim().is_empty() || line.starts_with(":") {
                        continue;
                    }

                    let data_line = if line.starts_with("data: ") {
                        &line[6..]
                    } else {
                        &line
                    };

                    if data_line.trim() == "[DONE]" {
                        break;
                    }

                    let event: AnthropicStreamEvent = match serde_json::from_str(data_line) {
                        Ok(event) => event,
                        Err(e) => {
                            debug!("Failed to parse SSE line: {} - Error: {}", data_line, e);
                            continue;
                        }
                    };

                    match process_stream_event(event, &mut active_tool_calls, &mut text_content) {
                        Ok(Some(response)) => yield Ok(response),
                        Ok(None) => {}, // Event processed but no response to emit
                        Err(e) => {
                            yield Err(e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    yield Err(e);
                    break;
                }
            }
        }

        for (_, (id, name, arguments)) in active_tool_calls {
            let tool_call = ToolCallRequest { id, name, arguments };
            yield Ok(LlmResponse::ToolRequestComplete { tool_call });
        }

        yield Ok(LlmResponse::Done);
    }
}

fn process_stream_event(
    event: AnthropicStreamEvent,
    active_tool_calls: &mut HashMap<u32, (String, String, String)>,
    _text_content: &mut String,
) -> Result<Option<LlmResponse>> {
    match event {
        AnthropicStreamEvent::MessageStart { data: _start_data } => {
            debug!("Message started");
            Ok(None)
        }
        AnthropicStreamEvent::ContentBlockStart { data: start_data } => {
            match start_data.content_block {
                ContentBlockStartData::Text { .. } => {
                    debug!("Text block started at index {}", start_data.index);
                    Ok(None)
                }
                ContentBlockStartData::ToolUse { id, name } => {
                    debug!("Tool use started: {} ({})", name, id);
                    active_tool_calls
                        .insert(start_data.index, (id.clone(), name.clone(), String::new()));
                    Ok(Some(LlmResponse::ToolRequestStart { id, name }))
                }
            }
        }
        AnthropicStreamEvent::ContentBlockDelta { data: delta_data } => match delta_data.delta {
            ContentBlockDeltaData::TextDelta { text } => {
                if !text.is_empty() {
                    Ok(Some(LlmResponse::Text { chunk: text }))
                } else {
                    Ok(None)
                }
            }
            ContentBlockDeltaData::InputJsonDelta { partial_json } => {
                if let Some((id, _name, arguments)) = active_tool_calls.get_mut(&delta_data.index) {
                    arguments.push_str(&partial_json);
                    Ok(Some(LlmResponse::ToolRequestArg {
                        id: id.clone(),
                        chunk: partial_json,
                    }))
                } else {
                    warn!(
                        "Received tool input delta for unknown tool call index: {}",
                        delta_data.index
                    );
                    Ok(None)
                }
            }
        },
        AnthropicStreamEvent::ContentBlockStop { data: stop_data } => {
            if let Some((id, name, arguments)) = active_tool_calls.remove(&stop_data.index) {
                let tool_call = ToolCallRequest {
                    id,
                    name,
                    arguments,
                };
                Ok(Some(LlmResponse::ToolRequestComplete { tool_call }))
            } else {
                debug!("Content block stopped at index {}", stop_data.index);
                Ok(None)
            }
        }
        AnthropicStreamEvent::MessageDelta { data: _delta_data } => {
            debug!("Message delta received");
            Ok(None)
        }
        AnthropicStreamEvent::MessageStop { data: _stop_data } => {
            debug!("Message stopped");
            Ok(None)
        }
        AnthropicStreamEvent::Error { data: error_data } => Err(color_eyre::eyre::eyre!(
            "Anthropic API error: {} - {}",
            error_data.error.error_type,
            error_data.error.message
        )),
        AnthropicStreamEvent::Ping => {
            debug!("Received ping event");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream;

    #[tokio::test]
    async fn test_process_text_stream() {
        let lines = vec![
            "data: {\"type\": \"message_start\", \"message\": {\"id\": \"msg_123\", \"type\": \"message\", \"role\": \"assistant\", \"content\": [], \"model\": \"claude-3\", \"stop_reason\": null, \"stop_sequence\": null, \"usage\": {\"input_tokens\": 10, \"output_tokens\": 0}}}".to_string(),
            "data: {\"type\": \"content_block_start\", \"index\": 0, \"content_block\": {\"type\": \"text\", \"text\": \"\"}}".to_string(),
            "data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \"Hello\"}}".to_string(),
            "data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \" world\"}}".to_string(),
            "data: {\"type\": \"content_block_stop\", \"index\": 0}".to_string(),
            "data: {\"type\": \"message_stop\"}".to_string(),
        ];

        let stream = tokio_stream::iter(lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_anthropic_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result.unwrap());
        }

        assert!(matches!(responses[0], LlmResponse::Start { .. }));
        assert!(matches!(responses[1], LlmResponse::Text { ref chunk } if chunk == "Hello"));
        assert!(matches!(responses[2], LlmResponse::Text { ref chunk } if chunk == " world"));
        assert!(matches!(responses[3], LlmResponse::Done));
    }

    #[tokio::test]
    async fn test_process_tool_use_stream() {
        let lines = vec![
            "data: {\"type\": \"message_start\", \"message\": {\"id\": \"msg_123\", \"type\": \"message\", \"role\": \"assistant\", \"content\": [], \"model\": \"claude-3\", \"stop_reason\": null, \"stop_sequence\": null, \"usage\": {\"input_tokens\": 10, \"output_tokens\": 0}}}".to_string(),
            "data: {\"type\": \"content_block_start\", \"index\": 0, \"content_block\": {\"type\": \"tool_use\", \"id\": \"tool_123\", \"name\": \"search\"}}".to_string(),
            "data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"input_json_delta\", \"partial_json\": \"{\\\"query\\\":\\\"test\\\"}\"}".to_string(),
            "data: {\"type\": \"content_block_stop\", \"index\": 0}".to_string(),
            "data: {\"type\": \"message_stop\"}".to_string(),
        ];

        let stream = tokio_stream::iter(lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_anthropic_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result.unwrap());
        }

        assert!(matches!(responses[0], LlmResponse::Start { .. }));
        assert!(
            matches!(responses[1], LlmResponse::ToolRequestStart { ref id, ref name } if id == "tool_123" && name == "search")
        );
        assert!(
            matches!(responses[2], LlmResponse::ToolRequestComplete { ref tool_call } if tool_call.id == "tool_123" && tool_call.name == "search")
        );
        assert!(matches!(responses[3], LlmResponse::Done));
    }

    #[tokio::test]
    async fn test_anthropic_stream_event_enum_deserialization() {
        use super::super::types::AnthropicStreamEvent;

        // Test message_start deserialization
        let message_start_json = r#"{"type": "message_start", "message": {"id": "msg_123", "type": "message", "role": "assistant", "content": [], "model": "claude-3", "stop_reason": null, "stop_sequence": null, "usage": {"input_tokens": 10, "output_tokens": 0}}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(message_start_json).unwrap();
        assert!(matches!(event, AnthropicStreamEvent::MessageStart { .. }));

        // Test content_block_start deserialization
        let content_block_start_json = r#"{"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(content_block_start_json).unwrap();
        assert!(matches!(
            event,
            AnthropicStreamEvent::ContentBlockStart { .. }
        ));

        // Test content_block_delta deserialization
        let content_block_delta_json = r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(content_block_delta_json).unwrap();
        assert!(matches!(
            event,
            AnthropicStreamEvent::ContentBlockDelta { .. }
        ));

        // Test ping deserialization
        let ping_json = r#"{"type": "ping"}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(ping_json).unwrap();
        assert!(matches!(event, AnthropicStreamEvent::Ping));

        // Test error deserialization
        let error_json = r#"{"type": "error", "error": {"type": "rate_limit_error", "message": "Rate limit exceeded"}}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(error_json).unwrap();
        assert!(matches!(event, AnthropicStreamEvent::Error { .. }));
    }
}
