use async_openai::types::CreateChatCompletionStreamResponse;
use async_stream;
use color_eyre::Result;
use std::collections::HashMap;
use tokio_stream::{Stream, StreamExt};
use tracing::debug;

use crate::types::{LlmMessage, ToolCall};

/// Common stream processing logic that handles tool call state tracking and event emission.
/// Works with standard async_openai CreateChatCompletionStreamResponse types.
pub fn process_completion_stream<E: Into<color_eyre::Report> + Send>(
    mut stream: impl Stream<Item = Result<CreateChatCompletionStreamResponse, E>> + Send + Unpin,
) -> impl Stream<Item = Result<LlmMessage>> + Send {
    async_stream::stream! {
        let message_id = uuid::Uuid::new_v4().to_string();
        yield Ok(LlmMessage::Start { message_id: message_id.clone() });

        let mut current_tool_id: Option<String> = None;
        let mut active_tool_calls: HashMap<String, (String, String)> = HashMap::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    if let Some(choice) = response.choices.first() {
                        let delta = &choice.delta;

                        // Handle content
                        if let Some(content) = &delta.content {
                            if !content.is_empty() {
                                // If we have a pending tool call and now we're getting content,
                                // complete the tool call first
                                if let Some(id) = current_tool_id.take() {
                                    if let Some((name, arguments)) = active_tool_calls.remove(&id) {
                                        let tool_call = ToolCall {
                                            id: id.clone(),
                                            name,
                                            arguments,
                                        };
                                        yield Ok(LlmMessage::ToolCallComplete { tool_call });
                                    }
                                }
                                yield Ok(LlmMessage::Content { chunk: content.clone() });
                            }
                        }

                        // Handle tool calls
                        if let Some(tool_calls) = &delta.tool_calls {
                            for tool_call in tool_calls {
                                if let Some(function) = &tool_call.function {
                                    // Tool call start
                                    if let Some(name) = &function.name {
                                        let id = tool_call.id.clone().unwrap_or_else(|| "tool_call_0".to_string());
                                        current_tool_id = Some(id.clone());
                                        active_tool_calls.insert(id.clone(), (name.clone(), String::new()));
                                        yield Ok(LlmMessage::ToolCallStart {
                                            id,
                                            name: name.clone(),
                                        });
                                    }

                                    // Tool call arguments
                                    if let Some(arguments) = &function.arguments {
                                        if !arguments.is_empty() {
                                            if let Some(id) = &current_tool_id {
                                                // Accumulate arguments in our state tracking
                                                if let Some((_, accumulated_args)) = active_tool_calls.get_mut(id) {
                                                    accumulated_args.push_str(arguments);
                                                }
                                                yield Ok(LlmMessage::ToolCallArgument {
                                                    id: id.clone(),
                                                    chunk: arguments.clone(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Handle finish reason - this indicates stream completion
                        if let Some(finish_reason) = &choice.finish_reason {
                            let finish_reason_str = format!("{finish_reason:?}");
                            debug!("Received finish reason: {}", finish_reason_str);

                            // Complete any pending tool call before ending
                            if let Some(id) = current_tool_id.take() {
                                if let Some((name, arguments)) = active_tool_calls.remove(&id) {
                                    let tool_call = ToolCall {
                                        id: id.clone(),
                                        name,
                                        arguments,
                                    };
                                    yield Ok(LlmMessage::ToolCallComplete { tool_call });
                                }
                            }

                            // End the stream for any finish reason
                            yield Ok(LlmMessage::Done);
                            break;
                        }
                    } else {
                        // No choices means stream is done
                        debug!("No choices in response, ending stream");
                        if let Some(id) = current_tool_id.take() {
                            if let Some((name, arguments)) = active_tool_calls.remove(&id) {
                                let tool_call = ToolCall {
                                    id: id.clone(),
                                    name,
                                    arguments,
                                };
                                yield Ok(LlmMessage::ToolCallComplete { tool_call });
                            }
                        }
                        yield Ok(LlmMessage::Done);
                        break;
                    }
                }
                Err(e) => {
                    yield Err(e.into());
                    break;
                }
            }
        }
    }
}
