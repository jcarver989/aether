use crate::llm::{ChatRequest, StreamChunk};
use crate::types::IsoString;
use crate::{llm::ChatMessage as LlmChatMessage, llm::LlmProvider, types::ChatMessage};
use async_openai::Chat;
use color_eyre::eyre::Result;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;

pub struct Agent2<T: LlmProvider> {
    llm: T,
    system_prompt: Option<String>,
    conversation_history: Vec<ChatMessage>,
    tx: Sender<ChatMessage>,
}

impl<T: LlmProvider> Agent2<T> {
    pub fn new(llm: T, tx: Sender<ChatMessage>, system_prompt: Option<String>) -> Self {
        let conversation_history = Vec<ChatMessage>::new();

        if let Some(ref system) = system_prompt {
            conversation_history.push(LlmChatMessage::System {
                content: system.clone(),
            });
        }

        Agent2 {
            llm,
            tx,
            conversation_history,
            system_prompt,
        }
    }

    pub async fn send_message(&mut self, content: &str) -> Result<()> {
        let message = ChatMessage::User {
            content: content.to_string(),
            timestamp: IsoString::now(),
        };

        self.conversation_history.push(message);

        let mut stream = self.llm.complete_stream_chunks(ChatRequest {
            messages: map_messages(&self.conversation_history),
            tools: Vec::new(),
            temperature: None,
        }).await?;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(StreamChunk::Content { content }) => {
                    agent.append_streaming_content(&content);
                }

                Ok(StreamChunk::Done => {
                    agent.finalize_streaming_message();
                    break;
                })

                Err(e) => {}
            }
        }

        Ok(())
    }
}

fn map_messages(conversation_history: &Vec<ChatMessage>) -> Vec<LlmChatMessage> {
    let mut llm_messages = Vec::new();

    let mut i = 0;
    while i < conversation_history.len() {
        let message = &conversation_history[i];

        match message {
            ChatMessage::System { content, .. } => {
                llm_messages.push(LlmChatMessage::System {
                    content: content.clone(),
                });
            }
            ChatMessage::User { content, .. } => {
                llm_messages.push(LlmChatMessage::User {
                    content: content.clone(),
                });
            }
            ChatMessage::Assistant { content, .. }
            | ChatMessage::AssistantStreaming { content, .. } => {
                // Look ahead for tool calls
                let mut tool_calls = Vec::new();
                let mut j = i + 1;

                while j < conversation_history.len() {
                    if let ChatMessage::ToolCall {
                        id, name, params, ..
                    } = &conversation_history[j]
                    {
                        if let Ok(arguments) = serde_json::from_str::<serde_json::Value>(params) {
                            tool_calls.push(crate::llm::provider::ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: arguments.to_string(),
                            });
                        }
                        j += 1;
                    } else {
                        break;
                    }
                }

                llm_messages.push(LlmChatMessage::Assistant {
                    content: content.clone(),
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                });

                i = j - 1;
            }
            ChatMessage::ToolResult {
                tool_call_id,
                content,
                ..
            } => {
                llm_messages.push(LlmChatMessage::Tool {
                    tool_call_id: tool_call_id.clone(),
                    content: content.clone(),
                });
            }
            ChatMessage::Tool { .. } | ChatMessage::ToolCall { .. } | ChatMessage::Error { .. } => {
                // Skip these in LLM context
            }
        }
        i += 1;
    }

    llm_messages
}
