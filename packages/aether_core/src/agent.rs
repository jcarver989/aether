use crate::llm::ChatRequest;
use crate::llm::LlmProvider;
use crate::mcp::McpClient;
use crate::types::{ChatMessage, IsoString, LlmMessage};
use async_stream::stream;
use futures::StreamExt;
use futures::pin_mut;
use tokio_stream::Stream;

#[derive(Debug, Clone)]
pub enum AgentEvent {
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

    pub async fn send_message(&mut self, content: &str) -> impl Stream<Item = AgentEvent> + Send {
        let user_message = ChatMessage::User {
            content: content.to_string(),
            timestamp: IsoString::now(),
        };

        self.messages.push(user_message);

        // Capture a mutable reference to messages to update them later
        let llm = &self.llm;
        let tools = self.mcp_client.get_tool_definitions();
        let messages = &mut self.messages;
        let messages_clone = messages.clone();

        stream! {
            let mut current_message_id = None;
            let mut accumulated_content = String::new();
            let mut completed_tool_calls = Vec::new();

            let llm_stream = llm.complete_stream_chunks(ChatRequest {
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
                            yield AgentEvent::MessageChunk {
                                message_id: message_id.clone(),
                                chunk,
                                is_complete: false,
                            };
                        }
                    }
                    Ok(LlmMessage::ToolCallStart { id, name }) => {
                        yield AgentEvent::ToolCallChunk {
                            tool_call_id: id,
                            name,
                            arguments: None,
                            result: None,
                            is_complete: false,
                        };
                    }
                    Ok(LlmMessage::ToolCallArgument { id, chunk }) => {
                        yield AgentEvent::ToolCallChunk {
                            tool_call_id: id,
                            name: String::new(), // Name will be available from the start event
                            arguments: Some(chunk),
                            result: None,
                            is_complete: false,
                        };
                    }
                    Ok(LlmMessage::ToolCallComplete { tool_call }) => {
                        completed_tool_calls.push(tool_call.clone());

                        yield AgentEvent::ToolCallChunk {
                            tool_call_id: tool_call.id,
                            name: tool_call.name,
                            arguments: None,
                            result: None,
                            is_complete: true,
                        };
                    }
                    Ok(LlmMessage::Done) => {
                        // Send final message chunk to indicate completion
                        if let Some(message_id) = &current_message_id {
                            yield AgentEvent::MessageChunk {
                                message_id: message_id.clone(),
                                chunk: String::new(),
                                is_complete: true,
                            };
                        }

                        // Add the completed assistant message to conversation history
                        messages.push(ChatMessage::Assistant {
                            content: accumulated_content,
                            timestamp: IsoString::now(),
                            tool_calls: completed_tool_calls,
                        });

                        break;
                    }
                    Ok(LlmMessage::Error { message }) => {
                        yield AgentEvent::Error { message };
                    }
                    Err(e) => {
                        yield AgentEvent::Error {
                            message: e.to_string(),
                        };
                    }
                }
            }
        }
    }
}
