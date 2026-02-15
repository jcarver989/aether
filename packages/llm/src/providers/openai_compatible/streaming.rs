use std::collections::HashMap;

use super::types::{ChatCompletionStreamResponse, ToolCallDelta};
use crate::{LlmError, LlmResponse, LlmResponseStream, Result, ToolCallRequest};
use async_openai::{Client, config::OpenAIConfig, types::chat::CreateChatCompletionRequest};
use async_stream;
use serde::Serialize;
use tokio_stream::{Stream, StreamExt};
use tracing::{debug, info, warn};

/// Creates a streaming response for OpenAI-compatible APIs.
/// This allows providers like OpenRouter and Z.ai to reuse the same streaming logic
/// while handling their API quirks through unified types.
pub fn create_custom_stream(
    client: &Client<OpenAIConfig>,
    request: CreateChatCompletionRequest,
) -> LlmResponseStream {
    create_custom_stream_generic(client, request)
}

/// Generic streaming function that accepts any serializable request type.
/// This enables providers to use custom request types while reusing the streaming logic.
pub fn create_custom_stream_generic<R: Serialize + Send + 'static>(
    client: &Client<OpenAIConfig>,
    request: R,
) -> LlmResponseStream {
    let client = client.clone();

    Box::pin(async_stream::stream! {
        let stream = match client
            .chat()
            .create_stream_byot::<R, ChatCompletionStreamResponse>(request)
            .await {
            Ok(stream) => stream,
            Err(e) => {
                yield Err(LlmError::ApiRequest(e.to_string()));
                return;
            }
        };

        let stream = stream.map(|result| result.map_err(|e| LlmError::ApiError(e.to_string())));

        for await item in process_compatible_stream(stream) {
            yield item;
        }
    })
}

pub fn process_compatible_stream<E: Into<LlmError> + Send>(
    mut stream: impl Stream<Item = std::result::Result<ChatCompletionStreamResponse, E>> + Send + Unpin,
) -> impl Stream<Item = Result<LlmResponse>> + Send {
    async_stream::stream! {
        let message_id = uuid::Uuid::new_v4().to_string();
        yield Ok(LlmResponse::Start { message_id });

        let mut tool_collector = ToolCallCollector::new();
        let mut chunk_count: u32 = 0;
        let mut had_text = false;
        let mut had_reasoning = false;
        let mut had_tool_calls = false;

        while let Some(result) = stream.next().await {
            match result {
                Ok(mut response) => {
                    chunk_count += 1;

                    if let Some(usage) = response.usage {
                        yield Ok(LlmResponse::Usage {
                            input_tokens: usage.prompt_tokens.max(0) as u32,
                            output_tokens: usage.completion_tokens.max(0) as u32,
                        });
                    }

                    if let Some(choice) = response.choices.pop() {
                        let delta = choice.delta;

                        if let Some(reasoning) = delta.reasoning_content
                            && !reasoning.is_empty() {
                                had_reasoning = true;
                                yield Ok(LlmResponse::Reasoning { chunk: reasoning });
                            }

                        if let Some(content) = delta.content
                            && !content.is_empty() {
                                had_text = true;
                                for tool_call in tool_collector.complete_all_tool_calls() {
                                    yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                                }
                                yield Ok(LlmResponse::Text { chunk: content });
                            }

                        if let Some(tool_calls) = delta.tool_calls {
                            had_tool_calls = true;
                            for tool_call in tool_calls {
                                let responses = tool_collector.handle_tool_call_delta(tool_call);
                                for response in responses {
                                    yield Ok(response);
                                }
                            }
                        }

                        if let Some(finish_reason) = choice.finish_reason {
                            let finish_reason_str = format!("{finish_reason:?}");
                            debug!("Received finish reason: {finish_reason_str}");

                            for tool_call in tool_collector.complete_all_tool_calls() {
                                yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                            }
                            // Continue stream to capture usage chunks after finish_reason.
                        }
                    } else {
                        // No choices in this chunk - could be:
                        // 1. Final usage-only chunk after finish_reason (OpenRouter)
                        // 2. Stream is done (some providers)
                        info!(chunk_count, had_text, had_reasoning, had_tool_calls, "No choices in chunk, ending stream");
                        for tool_call in tool_collector.complete_all_tool_calls() {
                            yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                        }
                        break;
                    }
                }
                Err(e) => {
                    yield Err(e.into());
                    break;
                }
            }
        }

        if chunk_count == 0 {
            warn!("Stream completed with zero chunks — provider returned an empty stream");
        } else {
            info!(chunk_count, had_text, had_reasoning, had_tool_calls, "Stream completed");
        }

        yield Ok(LlmResponse::Done);
    }
}

struct ToolCallCollector {
    active_tool_calls: HashMap<i32, (String, String, String)>,
}

impl ToolCallCollector {
    fn new() -> Self {
        Self {
            active_tool_calls: HashMap::new(),
        }
    }

    fn handle_tool_call_delta(&mut self, tool_call: ToolCallDelta) -> Vec<LlmResponse> {
        let mut responses = Vec::new();
        let ToolCallDelta {
            index,
            id,
            function,
            ..
        } = tool_call;

        if let Some(function) = function {
            if let Some(name) = function.name {
                let id = id.unwrap_or_else(|| format!("tool_call_{index}"));
                self.start_tool_call(index, id.clone(), name.clone());
                responses.push(LlmResponse::ToolRequestStart { id, name });
            }

            if let Some(arguments) = function.arguments
                && !arguments.is_empty()
                && let Some(id) = self.add_arguments(index, &arguments)
            {
                responses.push(LlmResponse::ToolRequestArg {
                    id,
                    chunk: arguments,
                });
            }
        }

        responses
    }

    fn complete_all_tool_calls(&mut self) -> Vec<ToolCallRequest> {
        self.active_tool_calls
            .drain()
            .map(|(_, (id, name, arguments))| ToolCallRequest {
                id,
                name,
                arguments,
            })
            .collect()
    }

    fn start_tool_call(&mut self, index: i32, id: String, name: String) {
        self.active_tool_calls
            .insert(index, (id, name, String::new()));
    }

    fn add_arguments(&mut self, index: i32, arguments: &str) -> Option<String> {
        if let Some((id, _, accumulated_args)) = self.active_tool_calls.get_mut(&index) {
            accumulated_args.push_str(arguments);
            return Some(id.clone());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::openai_compatible::types::{
        ChatCompletionStreamChoice, ChatCompletionStreamResponseDelta, FinishReason,
        FunctionCallDelta,
    };
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_process_compatible_stream_emits_reasoning_chunks() {
        let stream_items = vec![Ok::<ChatCompletionStreamResponse, std::io::Error>(
            ChatCompletionStreamResponse {
                id: "chunk_1".to_string(),
                choices: vec![ChatCompletionStreamChoice {
                    index: 0,
                    delta: ChatCompletionStreamResponseDelta {
                        role: None,
                        content: None,
                        reasoning_content: Some("thinking".to_string()),
                        tool_calls: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                created: 1,
                model: "kimi-k2.5".to_string(),
                system_fingerprint: None,
                object: "chat.completion.chunk".to_string(),
                usage: None,
            },
        )];

        let mut processed = Box::pin(process_compatible_stream(tokio_stream::iter(stream_items)));

        let mut events = Vec::new();
        while let Some(event) = processed.next().await {
            events.push(event.unwrap());
        }

        assert!(matches!(events[0], LlmResponse::Start { .. }));
        assert!(matches!(
            events[1],
            LlmResponse::Reasoning { ref chunk } if chunk == "thinking"
        ));
        assert!(matches!(events.last(), Some(LlmResponse::Done)));
    }

    #[tokio::test]
    async fn test_process_compatible_stream_handles_tool_calls() {
        let stream_items = vec![Ok::<ChatCompletionStreamResponse, std::io::Error>(
            ChatCompletionStreamResponse {
                id: "chunk_1".to_string(),
                choices: vec![ChatCompletionStreamChoice {
                    index: 0,
                    delta: ChatCompletionStreamResponseDelta {
                        role: None,
                        content: None,
                        reasoning_content: None,
                        tool_calls: Some(vec![ToolCallDelta {
                            index: 0,
                            id: Some("call_1".to_string()),
                            tool_type: Some("function".to_string()),
                            function: Some(FunctionCallDelta {
                                name: Some("tool".to_string()),
                                arguments: Some("{}".to_string()),
                            }),
                        }]),
                    },
                    finish_reason: Some(FinishReason::ToolCalls),
                    logprobs: None,
                }],
                created: 1,
                model: "kimi-k2.5".to_string(),
                system_fingerprint: None,
                object: "chat.completion.chunk".to_string(),
                usage: None,
            },
        )];

        let mut processed = Box::pin(process_compatible_stream(tokio_stream::iter(stream_items)));

        let mut events = Vec::new();
        while let Some(event) = processed.next().await {
            events.push(event.unwrap());
        }

        assert!(events.iter().any(|e| matches!(e, LlmResponse::ToolRequestStart { id, name } if id == "call_1" && name == "tool")));
        assert!(events.iter().any(|e| matches!(e, LlmResponse::ToolRequestComplete { tool_call } if tool_call.id == "call_1" && tool_call.arguments == "{}")));
        assert!(matches!(events.last(), Some(LlmResponse::Done)));
    }
}
