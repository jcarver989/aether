use super::types::{ContentBlockDeltaData, ContentBlockStartData, StreamEvent};
use crate::llm::{LlmError, LlmResponse, Result, ToolCallRequest};
use async_stream;
use futures::Stream;
use std::collections::HashMap;
use tokio_stream::StreamExt;
use tracing::{debug, warn};

pub fn process_anthropic_stream<T: Stream<Item = Result<String>> + Send + Sync + Unpin>(
    stream: T,
) -> impl Stream<Item = Result<LlmResponse>> + Send {
    async_stream::stream! {
        let message_id = uuid::Uuid::new_v4().to_string();
        yield Ok(LlmResponse::Start { message_id });

        let mut active_tool_calls: HashMap<String, (String, String)> = HashMap::new();
        let mut index_to_id: HashMap<u32, String> = HashMap::new();
        let mut stream = Box::pin(stream);

        while let Some(result) = stream.next().await {
            match result {
                Ok(line) => {
                    if line.trim().is_empty() || line.starts_with(":") {
                        continue;
                    }

                    let data_line = line.strip_prefix("data: ").unwrap_or(&line);

                    if data_line.trim() == "[DONE]" {
                        break;
                    }

                    let event: StreamEvent = match serde_json::from_str(data_line) {
                        Ok(event) => event,
                        Err(e) => {
                            debug!("Failed to parse SSE line: {} - Error: {}", data_line, e);
                            continue;
                        }
                    };

                    match process_stream_event(event, &mut active_tool_calls, &mut index_to_id) {
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

        for (id, (name, arguments)) in active_tool_calls {
            let tool_call = ToolCallRequest { id, name, arguments };
            yield Ok(LlmResponse::ToolRequestComplete { tool_call });
        }

        yield Ok(LlmResponse::Done);
    }
}

fn process_stream_event(
    event: StreamEvent,
    active_tool_calls: &mut HashMap<String, (String, String)>,
    index_to_id: &mut HashMap<u32, String>,
) -> Result<Option<LlmResponse>> {
    use StreamEvent::*;
    match event {
        MessageStart { data: start_data } => {
            debug!("Message started");
            // Emit usage from message_start event
            let usage = &start_data.message.usage;
            Ok(Some(LlmResponse::Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
            }))
        }

        ContentBlockStart { data: start_data } => match start_data.content_block {
            ContentBlockStartData::Text { .. } => {
                debug!("Text block started at index {}", start_data.index);
                Ok(None)
            }
            ContentBlockStartData::ToolUse { id, name } => {
                debug!("Tool use started: {} ({})", name, id);
                index_to_id.insert(start_data.index, id.clone());
                active_tool_calls.insert(id.clone(), (name.clone(), String::new()));
                Ok(Some(LlmResponse::ToolRequestStart { id, name }))
            }
        },

        ContentBlockDelta { data: delta_data } => match delta_data.delta {
            ContentBlockDeltaData::TextDelta { text } => {
                if !text.is_empty() {
                    Ok(Some(LlmResponse::Text { chunk: text }))
                } else {
                    Ok(None)
                }
            }

            ContentBlockDeltaData::InputJsonDelta { partial_json } => {
                if let Some(id) = index_to_id.get(&delta_data.index) {
                    if let Some((_name, arguments)) = active_tool_calls.get_mut(id) {
                        arguments.push_str(&partial_json);
                        Ok(Some(LlmResponse::ToolRequestArg {
                            id: id.clone(),
                            chunk: partial_json,
                        }))
                    } else {
                        warn!("Received tool input delta for unknown tool call id: {}", id);
                        Ok(None)
                    }
                } else {
                    warn!(
                        "Received tool input delta for unknown tool call index: {}",
                        delta_data.index
                    );
                    Ok(None)
                }
            }
        },
        ContentBlockStop { data: stop_data } => {
            if let Some(id) = index_to_id.remove(&stop_data.index) {
                if let Some((name, arguments)) = active_tool_calls.remove(&id) {
                    let tool_call = ToolCallRequest {
                        id,
                        name,
                        arguments,
                    };
                    Ok(Some(LlmResponse::ToolRequestComplete { tool_call }))
                } else {
                    debug!(
                        "Content block stopped but tool call not found for id: {}",
                        id
                    );
                    Ok(None)
                }
            } else {
                debug!("Content block stopped at index {}", stop_data.index);
                Ok(None)
            }
        }

        MessageDelta { data: delta_data } => {
            debug!("Message delta received");
            // Emit cumulative output_tokens if available
            if let Some(usage) = &delta_data.delta.usage {
                Ok(Some(LlmResponse::Usage {
                    input_tokens: 0, // Already reported in message_start
                    output_tokens: usage.output_tokens,
                }))
            } else {
                Ok(None)
            }
        }

        MessageStop { data: _stop_data } => {
            debug!("Message stopped");
            Ok(None)
        }

        Error { data: error_data } => Err(LlmError::ApiError(format!(
            "Anthropic API error: {} - {}",
            error_data.error.error_type, error_data.error.message
        ))),

        Ping => {
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
        assert!(matches!(responses[1], LlmResponse::Usage { input_tokens: 10, output_tokens: 0 }));
        assert!(matches!(responses[2], LlmResponse::Text { ref chunk } if chunk == "Hello"));
        assert!(matches!(responses[3], LlmResponse::Text { ref chunk } if chunk == " world"));
        assert!(matches!(responses[4], LlmResponse::Done));
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
        assert!(matches!(responses[1], LlmResponse::Usage { input_tokens: 10, output_tokens: 0 }));
        assert!(
            matches!(responses[2], LlmResponse::ToolRequestStart { ref id, ref name } if id == "tool_123" && name == "search")
        );
        assert!(
            matches!(responses[3], LlmResponse::ToolRequestComplete { ref tool_call } if tool_call.id == "tool_123" && tool_call.name == "search")
        );
        assert!(matches!(responses[4], LlmResponse::Done));
    }

    #[tokio::test]
    async fn test_multiple_tool_calls_with_same_index() {
        // This test demonstrates the issue with index-based tracking
        // If multiple tool calls happen to have the same index (which shouldn't happen
        // but could theoretically), the current implementation would overwrite them
        let lines = vec![
            r#"data: {"type": "message_start", "message": {"id": "msg_123", "type": "message", "role": "assistant", "content": [], "model": "claude-3", "stop_reason": null, "stop_sequence": null, "usage": {"input_tokens": 10, "output_tokens": 0}}}"#.to_string(),
            r#"data: {"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "tool_123", "name": "search"}}"#.to_string(),
            r#"data: {"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"query\":\"test1\"}"}}"#.to_string(),
            r#"data: {"type": "content_block_stop", "index": 0}"#.to_string(),
            // Another tool call with different ID but same index (simulating potential edge case)
            r#"data: {"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "tool_456", "name": "calculate"}}"#.to_string(),
            r#"data: {"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"expression\":\"2+2\"}"}}"#.to_string(),
            r#"data: {"type": "content_block_stop", "index": 0}"#.to_string(),
            r#"data: {"type": "message_stop"}"#.to_string(),
        ];

        let stream = tokio_stream::iter(lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_anthropic_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result.unwrap());
        }

        // We should get both tool calls, but with index-based tracking we might lose one
        let tool_starts: Vec<_> = responses
            .iter()
            .filter(|r| matches!(r, LlmResponse::ToolRequestStart { .. }))
            .collect();
        let tool_completes: Vec<_> = responses
            .iter()
            .filter(|r| matches!(r, LlmResponse::ToolRequestComplete { .. }))
            .collect();

        // With ID-based tracking, we should get both tool calls
        assert_eq!(tool_starts.len(), 2);
        assert_eq!(tool_completes.len(), 2);
    }

    #[tokio::test]
    async fn test_anthropic_stream_event_enum_deserialization() {
        use super::super::types::StreamEvent;

        // Test message_start deserialization
        let message_start_json = r#"{"type": "message_start", "message": {"id": "msg_123", "type": "message", "role": "assistant", "content": [], "model": "claude-3", "stop_reason": null, "stop_sequence": null, "usage": {"input_tokens": 10, "output_tokens": 0}}}"#;
        let event: StreamEvent = serde_json::from_str(message_start_json).unwrap();
        assert!(matches!(event, StreamEvent::MessageStart { .. }));

        // Test content_block_start deserialization
        let content_block_start_json = r#"{"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}"#;
        let event: StreamEvent = serde_json::from_str(content_block_start_json).unwrap();
        assert!(matches!(event, StreamEvent::ContentBlockStart { .. }));

        // Test content_block_delta deserialization
        let content_block_delta_json = r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#;
        let event: StreamEvent = serde_json::from_str(content_block_delta_json).unwrap();
        assert!(matches!(event, StreamEvent::ContentBlockDelta { .. }));

        // Test ping deserialization
        let ping_json = r#"{"type": "ping"}"#;
        let event: StreamEvent = serde_json::from_str(ping_json).unwrap();
        assert!(matches!(event, StreamEvent::Ping));

        // Test error deserialization
        let error_json = r#"{"type": "error", "error": {"type": "rate_limit_error", "message": "Rate limit exceeded"}}"#;
        let event: StreamEvent = serde_json::from_str(error_json).unwrap();
        assert!(matches!(event, StreamEvent::Error { .. }));
    }
}
