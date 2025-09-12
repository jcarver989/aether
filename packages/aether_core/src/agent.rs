use crate::llm::ChatRequest;
use crate::llm::LlmProvider;
use crate::mcp::McpClient;
use crate::types::{ChatMessage, IsoString, LlmMessage};
use async_stream::stream;
use futures::StreamExt;
use futures::pin_mut;
use tokio_stream::Stream;

#[derive(Debug, Clone)]
pub enum AgentMessage {
    MessageChunk {
        message_id: String,
        chunk: String,
        is_complete: bool,
    },

    ToolCallChunk {
        tool_call_id: String,
        name: String,
        arguments: Option<String>,
        result: Option<String>,
        is_complete: bool,
    },

    Error {
        message: String,
    },
}

pub struct Agent<T: LlmProvider> {
    llm: T,
    mcp_client: McpClient,
    messages: Vec<ChatMessage>,
}

impl<T: LlmProvider> Agent<T> {
    pub fn new(llm: T, mcp_client: McpClient, system_prompt: Option<String>) -> Self {
        let mut messages = Vec::new();

        if let Some(system_prompt) = &system_prompt {
            messages.push(ChatMessage::System {
                content: system_prompt.clone(),
                timestamp: IsoString::now(),
            });
        }

        Agent {
            llm,
            mcp_client,
            messages,
        }
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub async fn send_message(&mut self, content: &str) -> impl Stream<Item = AgentMessage> + Send {
        let user_message = ChatMessage::User {
            content: content.to_string(),
            timestamp: IsoString::now(),
        };

        self.messages.push(user_message);
        self.run_agent_loop().await
    }

    async fn run_agent_loop(&mut self) -> impl Stream<Item = AgentMessage> + Send {
        const MAX_ITERATIONS: usize = 10;
        let mut n_iterations = 0;

        stream! {
            loop {
                if n_iterations >= MAX_ITERATIONS {
                    yield AgentMessage::Error {
                        message: "Maximum recursion depth reached".to_string(),
                    };
                    break;
                }

                let tools = self.mcp_client.get_tool_definitions();
                let messages_clone = self.messages.clone();

                let mut current_message_id = None;
                let mut accumulated_content = String::new();
                let mut completed_tool_calls: Vec<(crate::types::ToolCallRequest, String)> = Vec::new();
                let mut has_tool_calls = false;

                let llm_stream = self.llm.complete_stream_chunks(ChatRequest {
                    messages: messages_clone,
                    tools,
                });

                pin_mut!(llm_stream);

                while let Some(event) = llm_stream.next().await {
                    match event {
                        Ok(LlmMessage::Start { message_id }) => {
                            current_message_id = Some(message_id);
                        }
                        Ok(LlmMessage::Content { chunk }) => {
                            accumulated_content.push_str(&chunk);

                            if let Some(message_id) = &current_message_id {
                                yield AgentMessage::MessageChunk {
                                    message_id: message_id.clone(),
                                    chunk,
                                    is_complete: false,
                                };
                            }
                        }
                        Ok(LlmMessage::ToolCallRequestStart { id, name }) => {
                            yield AgentMessage::ToolCallChunk {
                                tool_call_id: id,
                                name,
                                arguments: None,
                                result: None,
                                is_complete: false,
                            };
                        }
                        Ok(LlmMessage::ToolCallRequestArg { id, chunk }) => {
                            yield AgentMessage::ToolCallChunk {
                                tool_call_id: id,
                                name: String::new(), // Name will be available from the start event
                                arguments: Some(chunk),
                                result: None,
                                is_complete: false,
                            };
                        }
                        Ok(LlmMessage::ToolCallRequestComplete { tool_call }) => {
                            let result_str = match serde_json::from_str(&tool_call.arguments) {
                                Ok(args) => {
                                    match self.mcp_client.execute_tool(&tool_call.name, args).await {
                                        Ok(result) => result.to_string(),
                                        Err(e) => format!("Tool execution failed: {}", e),
                                    }
                                }
                                Err(e) => format!("Invalid tool arguments: {}", e),
                            };

                            // Store tool result but don't add to messages yet - wait for LLM Done
                            yield AgentMessage::ToolCallChunk {
                                tool_call_id: tool_call.id.clone(),
                                name: tool_call.name.clone(),
                                arguments: None,
                                result: Some(result_str.clone()),
                                is_complete: true,
                            };

                            // Store the tool call and result for later
                            completed_tool_calls.push((tool_call, result_str));
                            has_tool_calls = true;
                        }
                        Ok(LlmMessage::Done) => {
                            // Send final message chunk to indicate completion
                            if let Some(message_id) = &current_message_id {
                                yield AgentMessage::MessageChunk {
                                    message_id: message_id.clone(),
                                    chunk: String::new(),
                                    is_complete: true,
                                };
                            }

                            // Add the completed assistant message to conversation history first
                            let tool_call_requests: Vec<_> = completed_tool_calls
                                .iter()
                                .map(|(tool_call, _)| tool_call.clone())
                                .collect();
                            
                            self.messages.push(ChatMessage::Assistant {
                                content: accumulated_content,
                                timestamp: IsoString::now(),
                                tool_calls: tool_call_requests,
                            });

                            // Then add tool results in the correct order
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
                        Ok(LlmMessage::Error { message }) => {
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
        }
    }
}
