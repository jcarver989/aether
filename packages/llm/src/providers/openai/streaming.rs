use async_openai::types::chat::CreateChatCompletionStreamResponse;
use async_stream;
use tokio_stream::{Stream, StreamExt};
use tracing::debug;

use crate::providers::tool_call_collector::ToolCallCollector;
use crate::{LlmError, LlmResponse, Result};

/// Common stream processing logic that handles tool call state tracking and event emission.
/// Works with standard async_openai CreateChatCompletionStreamResponse types.
pub fn process_completion_stream<E: Into<LlmError> + Send>(
    mut stream: impl Stream<Item = std::result::Result<CreateChatCompletionStreamResponse, E>>
    + Send
    + Unpin,
) -> impl Stream<Item = Result<LlmResponse>> + Send {
    async_stream::stream! {
        let message_id = uuid::Uuid::new_v4().to_string();
        yield Ok(LlmResponse::Start { message_id });

        let mut collector = ToolCallCollector::<u32>::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(mut response) => {
                    // Emit usage information if available
                    // This must be checked on every chunk since usage may come
                    // in a separate final chunk after finish_reason
                    if let Some(usage) = response.usage {
                        yield Ok(LlmResponse::Usage {
                            input_tokens: usage.prompt_tokens,
                            output_tokens: usage.completion_tokens,
                        });
                    }

                    if let Some(choice) = response.choices.pop() {
                        let delta = choice.delta;

                        if let Some(content) = delta.content
                            && !content.is_empty() {
                                // If we have pending tool calls and now we're getting content,
                                // complete all tool calls first
                                for tool_call in collector.complete_all() {
                                    yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                                }
                                yield Ok(LlmResponse::Text { chunk: content });
                            }

                        if let Some(tool_calls) = delta.tool_calls {
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

                            for tool_call in collector.complete_all() {
                                yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                            }
                            // Don't break yet - continue to capture usage from subsequent chunks
                            // OpenRouter sends usage in the last SSE message after finish_reason
                            // See: https://openrouter.ai/docs/guides/usage-accounting
                        }
                    } else {
                        // No choices in this chunk - could be:
                        // 1. Final usage-only chunk after finish_reason (OpenRouter)
                        // 2. Stream is done (some providers)
                        // We already extracted usage above if present
                        debug!("No choices in response, ending stream");
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

        yield Ok(LlmResponse::Done);
    }
}
