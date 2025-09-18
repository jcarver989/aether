use async_openai::types::{ChatCompletionMessageToolCallChunk, CreateChatCompletionStreamResponse};
use async_stream;
use std::collections::HashMap;
use tokio_stream::{Stream, StreamExt};
use tracing::debug;

use crate::llm::{LlmError, Result};
use crate::types::{LlmResponse, ToolCallRequest};

/// Common stream processing logic that handles tool call state tracking and event emission.
/// Works with standard async_openai CreateChatCompletionStreamResponse types.
pub fn process_completion_stream<E: Into<LlmError> + Send>(
    mut stream: impl Stream<Item = std::result::Result<CreateChatCompletionStreamResponse, E>>
    + Send
    + Unpin,
) -> impl Stream<Item = Result<LlmResponse>> + Send {
    async_stream::stream! {
        let message_id = uuid::Uuid::new_v4().to_string();
        yield Ok(LlmResponse::Start { message_id: message_id.clone() });

        let mut tool_collector = ToolCallCollector::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    if let Some(choice) = response.choices.first() {
                        let delta = &choice.delta;

                        if let Some(content) = &delta.content {
                            if !content.is_empty() {
                                // If we have pending tool calls and now we're getting content,
                                // complete all tool calls first
                                for tool_call in tool_collector.complete_all_tool_calls() {
                                    yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                                }
                                yield Ok(LlmResponse::Text { chunk: content.clone() });
                            }
                        }

                        if let Some(tool_calls) = &delta.tool_calls {
                            for tool_call in tool_calls {
                                let responses = tool_collector.handle_tool_call_delta(tool_call);
                                for response in responses {
                                    yield Ok(response);
                                }
                            }
                        }

                        if let Some(finish_reason) = &choice.finish_reason {
                            let finish_reason_str = format!("{finish_reason:?}");
                            debug!("Received finish reason: {}", finish_reason_str);

                            for tool_call in tool_collector.complete_all_tool_calls() {
                                yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                            }

                            yield Ok(LlmResponse::Done);
                            break;
                        }
                    } else {
                        // No choices means stream is done
                        debug!("No choices in response, ending stream");
                        for tool_call in tool_collector.complete_all_tool_calls() {
                            yield Ok(LlmResponse::ToolRequestComplete { tool_call });
                        }
                        yield Ok(LlmResponse::Done);
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

struct ToolCallCollector {
    active_tool_calls: HashMap<u32, (String, String, String)>,
}

impl ToolCallCollector {
    fn new() -> Self {
        Self {
            active_tool_calls: HashMap::new(),
        }
    }

    pub fn handle_tool_call_delta(
        &mut self,
        tool_call: &ChatCompletionMessageToolCallChunk,
    ) -> Vec<LlmResponse> {
        let mut responses = Vec::new();
        let index = tool_call.index;

        if let Some(function) = &tool_call.function {
            if let Some(name) = &function.name {
                let id = tool_call
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("tool_call_{}", index));
                self.start_tool_call(index, id.clone(), name.clone());
                responses.push(LlmResponse::ToolRequestStart {
                    id,
                    name: name.clone(),
                });
            }

            if let Some(arguments) = &function.arguments {
                if !arguments.is_empty() {
                    if let Some(id) = self.add_arguments(index, arguments) {
                        responses.push(LlmResponse::ToolRequestArg {
                            id,
                            chunk: arguments.clone(),
                        });
                    }
                }
            }
        }

        responses
    }

    pub fn complete_all_tool_calls(&mut self) -> Vec<ToolCallRequest> {
        let mut completed = Vec::new();
        for (_, (id, name, arguments)) in self.active_tool_calls.drain() {
            completed.push(ToolCallRequest {
                id,
                name,
                arguments,
            });
        }
        completed
    }

    fn start_tool_call(&mut self, index: u32, id: String, name: String) {
        self.active_tool_calls
            .insert(index, (id, name, String::new()));
    }

    fn add_arguments(&mut self, index: u32, arguments: &str) -> Option<String> {
        if let Some((id, _, accumulated_args)) = self.active_tool_calls.get_mut(&index) {
            accumulated_args.push_str(arguments);
            return Some(id.clone());
        }
        None
    }
}
