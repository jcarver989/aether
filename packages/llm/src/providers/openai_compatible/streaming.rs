use super::types::ChatCompletionStreamResponse;
use super::types::FinishReason;
use crate::providers::tool_call_collector::ToolCallCollector;
use crate::{LlmError, LlmResponse, LlmResponseStream, Result, StopReason};
use async_openai::{Client, config::OpenAIConfig};
use async_stream;
use serde::Serialize;
use tokio_stream::{Stream, StreamExt};
use tracing::{debug, info, warn};

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
                warn!("create_stream_byot failed: {e}");
                yield Err(LlmError::ApiRequest(e.to_string()));
                return;
            }
        };

        let stream = stream.map(|result| {
            if let Err(ref e) = result {
                warn!("Stream error from API: {e}");
            }
            result.map_err(|e| LlmError::ApiError(e.to_string()))
        });

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

        let mut collector = ToolCallCollector::<i32>::new();
        let mut chunk_count: u32 = 0;
        let mut had_text = false;
        let mut had_reasoning = false;
        let mut had_tool_calls = false;
        let mut last_stop_reason: Option<StopReason> = None;

        while let Some(result) = stream.next().await {
            match result {
                Ok(mut response) => {
                    chunk_count += 1;

                    if let Some(usage) = response.usage {
                        yield Ok(LlmResponse::Usage { tokens: usage.into() });
                    }

                    if let Some(choice) = response.choices.pop() {
                        let delta = choice.delta;

                        if let Some(reasoning) = delta.reasoning_content
                            && !reasoning.is_empty() {
                                had_reasoning = true;
                                yield Ok(LlmResponse::Reasoning {
                                    chunk: reasoning,
                                });
                            }

                        if let Some(content) = delta.content
                            && !content.is_empty() {
                                had_text = true;
                                for tool_call in collector.complete_all() {
                                    yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                                }
                                yield Ok(LlmResponse::Text { chunk: content });
                            }

                        if let Some(tool_calls) = delta.tool_calls {
                            had_tool_calls = true;
                            for tc in tool_calls {
                                let (id, name, args) = match tc.function {
                                    Some(f) => (tc.id, f.name, f.arguments),
                                    None => (tc.id, None, None),
                                };
                                for response in collector.handle_delta(tc.index, id, name, args) {
                                    yield Ok(response);
                                }
                            }
                        }

                        if let Some(finish_reason) = choice.finish_reason {
                            let finish_reason_str = format!("{finish_reason:?}");
                            debug!("Received finish reason: {finish_reason_str}");
                            last_stop_reason = Some(map_openai_compatible_finish_reason(finish_reason));

                            for tool_call in collector.complete_all() {
                                yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                            }
                            // Continue stream to capture usage chunks after finish_reason.
                        }
                    } else {
                        // No choices in this chunk - could be:
                        // 1. Final usage-only chunk after finish_reason (OpenRouter)
                        // 2. Stream is done (some providers)
                        info!(chunk_count, had_text, had_reasoning, had_tool_calls, "No choices in chunk, ending stream");
                        for tool_call in collector.complete_all() {
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

        yield Ok(LlmResponse::Done {
            stop_reason: last_stop_reason,
        });
    }
}

fn map_openai_compatible_finish_reason(reason: FinishReason) -> StopReason {
    match reason {
        FinishReason::Stop => StopReason::EndTurn,
        FinishReason::Length | FinishReason::ModelContextWindowExceeded => StopReason::Length,
        FinishReason::ToolCalls => StopReason::ToolCalls,
        FinishReason::ContentFilter => StopReason::ContentFilter,
        FinishReason::FunctionCall => StopReason::FunctionCall,
        FinishReason::Error | FinishReason::NetworkError => StopReason::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokenUsage;
    use crate::providers::openai_compatible::types::{
        ChatCompletionStreamChoice, ChatCompletionStreamResponseDelta, CompletionTokensDetails, FinishReason,
        FunctionCallDelta, PromptTokensDetails, ToolCallDelta, Usage,
    };
    use tokio_stream::StreamExt;

    fn usage_chunk(usage: Usage) -> ChatCompletionStreamResponse {
        ChatCompletionStreamResponse {
            id: "chunk_usage".to_string(),
            choices: vec![],
            created: 1,
            model: "test".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: Some(usage),
        }
    }

    async fn collect_first_usage(stream: Vec<ChatCompletionStreamResponse>) -> Option<TokenUsage> {
        let stream_items =
            stream.into_iter().map(Ok::<ChatCompletionStreamResponse, std::io::Error>).collect::<Vec<_>>();
        let mut processed = Box::pin(process_compatible_stream(tokio_stream::iter(stream_items)));
        let mut events = Vec::new();
        while let Some(event) = processed.next().await {
            events.push(event.unwrap());
        }
        events.into_iter().find_map(|e| match e {
            LlmResponse::Usage { tokens } => Some(tokens),
            _ => None,
        })
    }

    #[tokio::test]
    async fn test_process_compatible_stream_emits_reasoning_chunks() {
        let stream_items = vec![Ok::<ChatCompletionStreamResponse, std::io::Error>(ChatCompletionStreamResponse {
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
        })];

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
        assert!(matches!(events.last(), Some(LlmResponse::Done { stop_reason: None })));
    }

    #[tokio::test]
    async fn test_process_compatible_stream_handles_tool_calls() {
        let stream_items = vec![Ok::<ChatCompletionStreamResponse, std::io::Error>(ChatCompletionStreamResponse {
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
        })];

        let mut processed = Box::pin(process_compatible_stream(tokio_stream::iter(stream_items)));

        let mut events = Vec::new();
        while let Some(event) = processed.next().await {
            events.push(event.unwrap());
        }

        assert!(
            events
                .iter()
                .any(|e| matches!(e, LlmResponse::ToolRequestStart { id, name } if id == "call_1" && name == "tool"))
        );
        assert!(events.iter().any(|e| matches!(e, LlmResponse::ToolRequestComplete { tool_call } if tool_call.id == "call_1" && tool_call.arguments == "{}")));
        assert!(matches!(events.last(), Some(LlmResponse::Done { stop_reason: Some(StopReason::ToolCalls) })));
    }

    #[tokio::test]
    async fn test_zai_shape_only_populates_cache_read() {
        let chunk = usage_chunk(Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_tokens_details: Some(PromptTokensDetails { cached_tokens: Some(30), ..Default::default() }),
            completion_tokens_details: None,
        });

        let tokens = collect_first_usage(vec![chunk]).await.expect("usage event");
        assert_eq!(
            tokens,
            TokenUsage { input_tokens: 100, output_tokens: 50, cache_read_tokens: Some(30), ..TokenUsage::default() }
        );
    }

    #[tokio::test]
    async fn test_openrouter_shape_populates_all_fields() {
        let chunk = usage_chunk(Usage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
            prompt_tokens_details: Some(PromptTokensDetails {
                cached_tokens: Some(100),
                cache_write_tokens: Some(50),
                audio_tokens: Some(10),
                video_tokens: Some(5),
            }),
            completion_tokens_details: Some(CompletionTokensDetails {
                reasoning_tokens: Some(300),
                audio_tokens: Some(8),
                accepted_prediction_tokens: Some(2),
                rejected_prediction_tokens: Some(1),
            }),
        });

        let tokens = collect_first_usage(vec![chunk]).await.expect("usage event");
        assert_eq!(
            tokens,
            TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read_tokens: Some(100),
                cache_creation_tokens: Some(50),
                input_audio_tokens: Some(10),
                input_video_tokens: Some(5),
                reasoning_tokens: Some(300),
                output_audio_tokens: Some(8),
                accepted_prediction_tokens: Some(2),
                rejected_prediction_tokens: Some(1),
            }
        );
    }

    #[tokio::test]
    async fn test_openai_shape_populates_input_audio_and_completion_details() {
        let chunk = usage_chunk(Usage {
            prompt_tokens: 200,
            completion_tokens: 100,
            total_tokens: 300,
            prompt_tokens_details: Some(PromptTokensDetails {
                cached_tokens: Some(80),
                audio_tokens: Some(12),
                ..Default::default()
            }),
            completion_tokens_details: Some(CompletionTokensDetails {
                reasoning_tokens: Some(40),
                accepted_prediction_tokens: Some(5),
                ..Default::default()
            }),
        });

        let tokens = collect_first_usage(vec![chunk]).await.expect("usage event");
        assert_eq!(tokens.cache_read_tokens, Some(80));
        assert_eq!(tokens.cache_creation_tokens, None);
        assert_eq!(tokens.input_audio_tokens, Some(12));
        assert_eq!(tokens.reasoning_tokens, Some(40));
        assert_eq!(tokens.accepted_prediction_tokens, Some(5));
    }

    #[tokio::test]
    async fn test_process_compatible_stream_maps_context_window_exceeded_to_length() {
        let response: ChatCompletionStreamResponse = serde_json::from_str(
            r#"{
                "id": "chunk_1",
                "created": 1,
                "model": "glm-5",
                "choices": [{
                    "index": 0,
                    "finish_reason": "model_context_window_exceeded",
                    "delta": {
                        "role": "assistant",
                        "content": ""
                    }
                }]
            }"#,
        )
        .expect("response should deserialize");

        let mut processed = Box::pin(process_compatible_stream(tokio_stream::iter(vec![Ok::<
            ChatCompletionStreamResponse,
            std::io::Error,
        >(response)])));

        let mut events = Vec::new();
        while let Some(event) = processed.next().await {
            events.push(event.unwrap());
        }

        assert!(matches!(events[0], LlmResponse::Start { .. }));
        assert!(matches!(events.last(), Some(LlmResponse::Done { stop_reason: Some(StopReason::Length) })));
    }
}
