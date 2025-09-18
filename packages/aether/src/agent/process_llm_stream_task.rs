use crate::agent::AgentMessage;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::types::ChatMessage;
use crate::types::IsoString;
use crate::types::LlmResponse;
use crate::types::ToolCallRequest;
use futures::StreamExt;
use futures::pin_mut;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex as TokioMutex, mpsc};
use tokio::task::JoinHandle;

pub struct ProcessLlmStreamTask {}

impl ProcessLlmStreamTask {
    pub fn run<T: ModelProvider + 'static>(
        llm: Arc<T>,
        context: Arc<Mutex<Context>>,
        tx: Sender<AgentMessage>,
        tool_call_tx: Sender<ToolCallRequest>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let response_stream = {
                let c = context.lock().unwrap();
                llm.stream_response(&c)
            };

            let model_name = llm.display_name();
            pin_mut!(response_stream);

            let mut current_message_id: Option<String> = None;
            let mut message_content = String::new();
            let mut tool_call_requests = Vec::<ToolCallRequest>::new();

            while let Some(event) = response_stream.next().await {
                use LlmResponse::*;
                match event {
                    Ok(Start { message_id }) => {
                        current_message_id = Some(message_id);
                    }

                    Ok(Text { chunk }) => {
                        message_content.push_str(&chunk);
                        if let Some(ref id) = current_message_id {
                            let _ = tx
                                .send(AgentMessage::Text {
                                    message_id: id.clone(),
                                    chunk,
                                    is_complete: false,
                                    model_name: model_name.clone(),
                                })
                                .await;
                        }
                    }

                    Ok(ToolRequestStart { id, name }) => {
                        let _ = tx
                            .send(AgentMessage::ToolCall {
                                tool_call_id: id,
                                name,
                                arguments: None,
                                result: None,
                                is_complete: false,
                                model_name: model_name.clone(),
                            })
                            .await;
                    }

                    Ok(ToolRequestArg { id, chunk }) => {
                        let _ = tx
                            .send(AgentMessage::ToolCall {
                                tool_call_id: id,
                                name: String::new(),
                                arguments: Some(chunk.to_string()),
                                result: None,
                                is_complete: false,
                                model_name: model_name.clone(),
                            })
                            .await;
                    }

                    Ok(ToolRequestComplete { tool_call }) => {
                        tracing::debug!(
                            "Tool request completed: {} ({})",
                            tool_call.name,
                            tool_call.id
                        );

                        tool_call_requests.push(tool_call.clone());
                        tracing::debug!(
                            "Sending tool call to executor: {} ({})",
                            tool_call.name,
                            tool_call.id
                        );
                        let send_result = tool_call_tx.send(tool_call.clone()).await;
                        tracing::debug!(
                            "Tool call send result for {} ({}): {:?}",
                            tool_call.name,
                            tool_call.id,
                            send_result
                        );

                        let _ = tx
                            .send(AgentMessage::ToolCall {
                                tool_call_id: tool_call.id.clone(),
                                name: tool_call.name.clone(),
                                arguments: Some(tool_call.arguments.clone()),
                                result: None,
                                is_complete: false,
                                model_name: model_name.clone(),
                            })
                            .await;
                    }

                    Ok(Done) => {
                        break;
                    }

                    Ok(Error { message }) => {
                        let _ = tx
                            .send(AgentMessage::Error {
                                message: message.to_string(),
                            })
                            .await;
                        return;
                    }

                    Err(e) => {
                        let _ = tx
                            .send(AgentMessage::Error {
                                message: e.to_string(),
                            })
                            .await;
                        return;
                    }
                }
            }

            if let Some(ref id) = current_message_id {
                context.lock().unwrap().add_message(ChatMessage::Assistant {
                    content: message_content.clone(),
                    timestamp: IsoString::now(),
                    tool_calls: tool_call_requests,
                });

                let _ = tx
                    .send(AgentMessage::Text {
                        message_id: id.clone(),
                        chunk: message_content,
                        is_complete: true,
                        model_name: model_name.clone(),
                    })
                    .await;
            }
        })
    }
}
