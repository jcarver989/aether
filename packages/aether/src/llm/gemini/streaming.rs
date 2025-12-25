//! SSE stream parser for CodeAssist API responses.
//!
//! The CodeAssist API streams responses in Server-Sent Events (SSE) format:
//! - Lines starting with `data: ` contain JSON payloads
//! - Empty lines delimit events
//! - Other lines (like `id:`) are ignored

use crate::llm::{LlmResponse, Result, ToolCallRequest};
use async_stream::stream;
use futures::Stream;
use tokio_stream::StreamExt;
use tracing::debug;
use uuid::Uuid;

use super::types::{CaGenerateContentResponse, ResponsePart};

/// Process a CodeAssist SSE stream and emit LlmResponse events
pub fn process_codeassist_stream<T: Stream<Item = Result<String>> + Send + Unpin>(
    raw_stream: T,
) -> impl Stream<Item = Result<LlmResponse>> + Send {
    stream! {
        let message_id = Uuid::new_v4().to_string();
        yield Ok(LlmResponse::Start { message_id });

        let mut stream = Box::pin(raw_stream);
        let mut buffered_lines: Vec<String> = Vec::new();
        let mut tool_call_counter = 0u32;

        while let Some(result) = stream.next().await {
            match result {
                Ok(line) => {
                    if line.starts_with("data: ") {
                        buffered_lines.push(line[6..].trim().to_string());
                    } else if line.is_empty() && !buffered_lines.is_empty() {
                        let json_str = buffered_lines.join("\n");
                        buffered_lines.clear();

                        match serde_json::from_str::<CaGenerateContentResponse>(&json_str) {
                            Ok(response) => {
                                for item in process_response_chunk(response, &mut tool_call_counter) {
                                    yield item;
                                }
                            }
                            Err(e) => {
                                debug!("Failed to parse CodeAssist SSE chunk: {} - {}", e, json_str);
                            }
                        }
                    }
                    // Ignore other lines (id:, comments, etc.)
                }
                Err(e) => {
                    yield Err(e);
                    break;
                }
            }
        }

        // Process any remaining buffered data
        if !buffered_lines.is_empty() {
            let json_str = buffered_lines.join("\n");
            if let Ok(response) = serde_json::from_str::<CaGenerateContentResponse>(&json_str) {
                for item in process_response_chunk(response, &mut tool_call_counter) {
                    yield item;
                }
            }
        }

        yield Ok(LlmResponse::Done);
    }
}

/// Process a single CodeAssist response chunk and emit LlmResponse events
fn process_response_chunk(
    response: CaGenerateContentResponse,
    tool_call_counter: &mut u32,
) -> Vec<Result<LlmResponse>> {
    let mut results = Vec::new();

    for candidate in response.response.candidates {
        if let Some(content) = candidate.content {
            for part in content.parts {
                match part {
                    ResponsePart::Text { text } => {
                        if !text.is_empty() {
                            results.push(Ok(LlmResponse::Text { chunk: text }));
                        }
                    }
                    ResponsePart::FunctionCall { function_call } => {
                        let id = format!("call_{}", *tool_call_counter);
                        *tool_call_counter += 1;

                        let arguments = serde_json::to_string(&function_call.args)
                            .unwrap_or_else(|_| "{}".to_string());

                        // Emit start, arg, and complete in sequence
                        results.push(Ok(LlmResponse::tool_request_start(
                            &id,
                            &function_call.name,
                        )));
                        results.push(Ok(LlmResponse::tool_request_arg(&id, &arguments)));
                        results.push(Ok(LlmResponse::ToolRequestComplete {
                            tool_call: ToolCallRequest {
                                id,
                                name: function_call.name,
                                arguments,
                            },
                        }));
                    }
                }
            }
        }
    }

    // Emit usage if present
    if let Some(usage) = response.response.usage_metadata {
        results.push(Ok(LlmResponse::Usage {
            input_tokens: usage.prompt_token_count.unwrap_or(0),
            output_tokens: usage.candidates_token_count.unwrap_or(0),
        }));
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream;

    #[tokio::test]
    async fn test_process_text_stream() {
        let lines = vec![
            "data: {\"response\":{\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Hello\"}]}}]}}".to_string(),
            "".to_string(),
            "data: {\"response\":{\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\" world!\"}]}}]}}".to_string(),
            "".to_string(),
        ];

        let stream = tokio_stream::iter(lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_codeassist_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result.unwrap());
        }

        assert!(matches!(responses[0], LlmResponse::Start { .. }));
        assert!(matches!(responses[1], LlmResponse::Text { ref chunk } if chunk == "Hello"));
        assert!(matches!(responses[2], LlmResponse::Text { ref chunk } if chunk == " world!"));
        assert!(matches!(responses[3], LlmResponse::Done));
    }

    #[tokio::test]
    async fn test_process_function_call_stream() {
        let lines = vec![
            r#"data: {"response":{"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"name":"get_weather","args":{"location":"NYC"}}}]}}]}}"#.to_string(),
            "".to_string(),
        ];

        let stream = tokio_stream::iter(lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_codeassist_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result.unwrap());
        }

        assert!(matches!(responses[0], LlmResponse::Start { .. }));
        assert!(
            matches!(responses[1], LlmResponse::ToolRequestStart { ref id, ref name } if id == "call_0" && name == "get_weather")
        );
        assert!(
            matches!(responses[2], LlmResponse::ToolRequestArg { ref id, .. } if id == "call_0")
        );
        assert!(
            matches!(responses[3], LlmResponse::ToolRequestComplete { ref tool_call } if tool_call.name == "get_weather")
        );
        assert!(matches!(responses[4], LlmResponse::Done));
    }

    #[tokio::test]
    async fn test_process_usage_metadata() {
        let lines = vec![
            r#"data: {"response":{"candidates":[{"content":{"role":"model","parts":[{"text":"Hi"}]}}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5}}}"#.to_string(),
            "".to_string(),
        ];

        let stream = tokio_stream::iter(lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_codeassist_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result.unwrap());
        }

        assert!(matches!(responses[0], LlmResponse::Start { .. }));
        assert!(matches!(responses[1], LlmResponse::Text { ref chunk } if chunk == "Hi"));
        assert!(matches!(
            responses[2],
            LlmResponse::Usage {
                input_tokens: 10,
                output_tokens: 5
            }
        ));
        assert!(matches!(responses[3], LlmResponse::Done));
    }

    #[tokio::test]
    async fn test_ignores_non_data_lines() {
        let lines = vec![
            "id: 123".to_string(),
            ": comment".to_string(),
            "data: {\"response\":{\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Hi\"}]}}]}}".to_string(),
            "".to_string(),
        ];

        let stream = tokio_stream::iter(lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_codeassist_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result.unwrap());
        }

        assert!(matches!(responses[0], LlmResponse::Start { .. }));
        assert!(matches!(responses[1], LlmResponse::Text { ref chunk } if chunk == "Hi"));
        assert!(matches!(responses[2], LlmResponse::Done));
    }

    #[tokio::test]
    async fn test_handles_multiline_json() {
        // SSE can have multi-line data (though rare)
        let lines = vec![
            "data: {\"response\":{".to_string(),
            "data: \"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Hi\"}]}}]}}".to_string(),
            "".to_string(),
        ];

        let stream = tokio_stream::iter(lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_codeassist_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result.unwrap());
        }

        // Multi-line JSON should be joined and parsed
        assert!(matches!(responses[0], LlmResponse::Start { .. }));
        assert!(matches!(responses[1], LlmResponse::Text { ref chunk } if chunk == "Hi"));
        assert!(matches!(responses[2], LlmResponse::Done));
    }

    #[tokio::test]
    async fn test_multiple_function_calls() {
        let lines = vec![
            r#"data: {"response":{"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"name":"tool_a","args":{}}},{"functionCall":{"name":"tool_b","args":{}}}]}}]}}"#.to_string(),
            "".to_string(),
        ];

        let stream = tokio_stream::iter(lines.into_iter().map(Ok));
        let mut response_stream = Box::pin(process_codeassist_stream(stream));

        let mut responses = Vec::new();
        while let Some(result) = response_stream.next().await {
            responses.push(result.unwrap());
        }

        // Should have Start, then 2 tool calls (each with start, arg, complete), then Done
        let tool_completes: Vec<_> = responses
            .iter()
            .filter(|r| matches!(r, LlmResponse::ToolRequestComplete { .. }))
            .collect();

        assert_eq!(tool_completes.len(), 2);
    }
}
