use crate::agent::AgentMessage;
use crate::agent::UserMessage;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager};
use crate::types::ToolCallRequest;
use crate::types::{ChatMessage, IsoString, LlmResponse};
use async_stream::stream;
use futures::StreamExt;
use futures::pin_mut;
use tokio::sync::mpsc;
use tokio_stream::Stream;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct Agent<T: ModelProvider> {
    llm: T,
    mcp_client: McpManager,
    messages: Vec<ChatMessage>,
    cancellation_token: CancellationToken,
    elicitation_receiver: mpsc::UnboundedReceiver<ElicitationRequest>,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub fn new(
        llm: T,
        mcp_client: McpManager,
        messages: Vec<ChatMessage>,
        elicitation_receiver: mpsc::UnboundedReceiver<ElicitationRequest>,
    ) -> Self {
        Self {
            llm,
            mcp_client,
            messages,
            cancellation_token: CancellationToken::new(),
            elicitation_receiver,
        }
    }

    pub async fn send(
        &mut self,
        message: UserMessage,
    ) -> (impl Stream<Item = AgentMessage> + Send, CancellationToken) {
        self.run_agent_loop(message).await
    }

    async fn run_agent_loop(
        &mut self,
        message: UserMessage,
    ) -> (impl Stream<Item = AgentMessage> + Send, CancellationToken) {
        const MAX_ITERATIONS: usize = 10_000;
        let mut n_iterations = 0;

        let cancellation_token = match message {
            UserMessage::Text { content } => {
                let user_message = ChatMessage::User {
                    content,
                    timestamp: IsoString::now(),
                };

                self.messages.push(user_message);
                self.cancellation_token = CancellationToken::new();
                self.cancellation_token.clone()
            }
            UserMessage::Cancel => {
                self.cancellation_token.cancel();
                self.cancellation_token.clone()
            }
        };

        let stream = stream! {
            if self.cancellation_token.is_cancelled() {
                yield AgentMessage::Cancelled {
                    message: "Operation was cancelled".to_string(),
                };
                return;
            }

            match self.mcp_client.discover_tools().await  {
                Ok(_) => {}
                Err(e) => {
                    yield AgentMessage::Error {
                        message: format!("Failed to discover tools: {}", e),
                    };
                    return
                }
            };

            loop {
                if self.cancellation_token.is_cancelled() {
                    yield AgentMessage::Cancelled {
                        message: "Operation was cancelled during agent loop".to_string(),
                    };
                    return;
                }
                if n_iterations >= MAX_ITERATIONS {
                    yield AgentMessage::Error {
                        message: "Maximum recursion depth reached".to_string(),
                    };
                    break;
                }

                while let Ok(elicitation_request) = self.elicitation_receiver.try_recv() {
                    let request_id = Uuid::new_v4().to_string();

                    yield AgentMessage::ElicitationRequest {
                        request_id,
                        request: elicitation_request.request,
                        response_sender: elicitation_request.response_sender,
                    };
                }

                let tools = self.mcp_client.get_tool_definitions();
                let messages_clone = self.messages.clone();

                let mut current_message_id = None;
                let mut accumulated_content = String::new();
                let mut completed_tool_calls: Vec<(ToolCallRequest, String)> = Vec::new();
                let mut has_tool_calls = false;

                let llm_stream = self.llm.stream_response(Context {
                    messages: messages_clone,
                    tools,
                });

                pin_mut!(llm_stream);

                while let Some(event) = llm_stream.next().await {
                    if self.cancellation_token.is_cancelled() {
                        yield AgentMessage::Cancelled {
                            message: "Operation was cancelled".to_string(),
                        };
                        return;
                    }

                    use LlmResponse::*;
                    match event {
                        Ok(Start { message_id }) => {
                            current_message_id = Some(message_id);
                        }
                        Ok(Text { chunk }) => {
                            accumulated_content.push_str(&chunk);

                            if let Some(message_id) = &current_message_id {
                                yield AgentMessage::Text {
                                    message_id: message_id.clone(),
                                    chunk,
                                    is_complete: false,
                                };
                            }
                        }
                        Ok(ToolRequestStart { id, name }) => {
                            yield AgentMessage::ToolCall {
                                tool_call_id: id,
                                name,
                                arguments: None,
                                result: None,
                                is_complete: false,
                            };
                        }
                        Ok(ToolRequestArg { id, chunk }) => {
                            yield AgentMessage::ToolCall {
                                tool_call_id: id,
                                name: String::new(), // Name will be available from the start event
                                arguments: Some(chunk),
                                result: None,
                                is_complete: false,
                            };
                        }
                        Ok(ToolRequestComplete { tool_call }) => {
                            // Execute tool with concurrent elicitation handling
                            let result_str = match serde_json::from_str(&tool_call.arguments) {
                                Ok(args) => {
                                    // Execute tool while monitoring for elicitation requests
                                    let execute_future = self.mcp_client.execute_tool(&tool_call.name, args);
                                    futures::pin_mut!(execute_future);

                                    let mut tool_result = None;
                                    while tool_result.is_none() {
                                        tokio::select! {
                                            // Check for tool completion
                                            result = &mut execute_future => {
                                                match result {
                                                    Ok(result) => tool_result = Some(result.to_string()),
                                                    Err(e) => tool_result = Some(format!("Tool execution failed: {}", e)),
                                                }
                                            }
                                            // Check for elicitation requests
                                            elicitation_request = self.elicitation_receiver.recv() => {
                                                if let Some(elicitation_request) = elicitation_request {
                                                    let request_id = Uuid::new_v4().to_string();
                                                    yield AgentMessage::ElicitationRequest {
                                                        request_id,
                                                        request: elicitation_request.request,
                                                        response_sender: elicitation_request.response_sender,
                                                    };
                                                }
                                            }
                                        }
                                    }

                                    tool_result.unwrap()
                                }
                                Err(e) => format!("Invalid tool arguments: {}", e),
                            };

                            yield AgentMessage::ToolCall {
                                tool_call_id: tool_call.id.clone(),
                                name: tool_call.name.clone(),
                                arguments: None,
                                result: Some(result_str.clone()),
                                is_complete: true,
                            };

                            completed_tool_calls.push((tool_call, result_str));
                            has_tool_calls = true;
                        }
                        Ok(Done) => {
                            if let Some(message_id) = &current_message_id {
                                yield AgentMessage::Text {
                                    message_id: message_id.clone(),
                                    chunk: String::new(),
                                    is_complete: true,
                                };
                            }

                            let tool_call_requests: Vec<_> = completed_tool_calls
                                .iter()
                                .map(|(tool_call, _)| tool_call.clone())
                                .collect();

                            self.messages.push(ChatMessage::Assistant {
                                content: accumulated_content,
                                timestamp: IsoString::now(),
                                tool_calls: tool_call_requests,
                            });

                            for (tool_call, result_str) in completed_tool_calls {
                                self.messages.push(ChatMessage::ToolCallResult {
                                    tool_call_id: tool_call.id,
                                    content: result_str,
                                    timestamp: IsoString::now(),
                                });
                            }

                            if has_tool_calls {
                                n_iterations += 1;
                                break;
                            } else {
                                return;
                            }
                        }
                        Ok(Error { message }) => {
                            yield AgentMessage::Error { message };
                            return;
                        }
                        Err(e) => {
                            yield AgentMessage::Error {
                                message: e.to_string(),
                            };
                            return;
                        }
                    }
                }
            }
        };

        (stream, cancellation_token)
    }
}
