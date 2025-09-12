use crate::llm::ChatRequest;
use crate::llm::LlmProvider;
use crate::tools::ToolRegistry;
use crate::types::{ChatMessage, IsoString, LlmMessage, ToolCall, ToolDefinition};
use async_stream::stream;
use futures::StreamExt;
use futures::pin_mut;
use std::collections::HashMap;
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
    tools: ToolRegistry,
    messages: Vec<ChatMessage>,
}

impl<T: LlmProvider> Agent<T> {
    pub fn new(llm: T, tools: ToolRegistry, system_prompt: Option<String>) -> Self {
        let mut messages = Vec::new();

        if let Some(system_prompt) = &system_prompt {
            messages.push(ChatMessage::System {
                content: system_prompt.clone(),
                timestamp: IsoString::now(),
            });
        }

        Agent {
            llm,
            tools,
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
        let tools = self.build_tool_definitions();
        let messages = &mut self.messages;
        let messages_clone = messages.clone();

        stream! {
            let mut current_message_id = None;
            let mut accumulated_content = String::new();
            let mut active_tool_calls: HashMap<String, (String, String)> = HashMap::new();
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
                        active_tool_calls.insert(id.clone(), (name.clone(), String::new()));

                        yield AgentEvent::ToolCallChunk {
                            tool_call_id: id,
                            name,
                            arguments: None,
                            result: None,
                            is_complete: false,
                        };
                    }
                    Ok(LlmMessage::ToolCallArgument { id, chunk }) => {
                        if let Some((_, arguments)) = active_tool_calls.get_mut(&id) {
                            arguments.push_str(&chunk);
                        }

                        let tool_name = active_tool_calls
                            .get(&id)
                            .map(|(name, _)| name.clone())
                            .unwrap_or_default();

                        yield AgentEvent::ToolCallChunk {
                            tool_call_id: id,
                            name: tool_name,
                            arguments: Some(chunk),
                            result: None,
                            is_complete: false,
                        };
                    }
                    Ok(LlmMessage::ToolCallComplete { id }) => {
                        if let Some((name, arguments)) = active_tool_calls.remove(&id) {
                            completed_tool_calls.push(ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments,
                            });

                            yield AgentEvent::ToolCallChunk {
                                tool_call_id: id,
                                name,
                                arguments: None,
                                result: None,
                                is_complete: true,
                            };
                        }
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

    fn build_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .list_tools()
            .into_iter()
            .filter_map(|tool_name| {
                let description = self.tools.get_tool_description(&tool_name)?;
                let parameters = self.tools.get_tool_parameters(&tool_name)?.clone();

                Some(ToolDefinition {
                    name: tool_name.clone(),
                    description,
                    parameters: parameters.to_string(),
                    server: self.tools.get_server_for_tool(&tool_name).cloned(),
                })
            })
            .collect()
    }
}
