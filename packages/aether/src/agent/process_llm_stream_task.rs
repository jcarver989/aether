use crate::agent::AgentMessage;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::types::LlmResponse;
use futures::StreamExt;
use futures::pin_mut;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tracing::Level;
use tracing::span;

pub async fn run_llm_stream_processor<T: ModelProvider + 'static>(
    llm: Arc<T>,
    context: Arc<Context>,
    tx: Sender<AgentMessage>,
) {
    let span = span!(Level::DEBUG, "process_llm_stream");
    let _guard = span.enter();

    let response_stream = llm.stream_response(&context);

    let model_name = llm.display_name();
    pin_mut!(response_stream);

    let mut current_message_id: Option<String> = None;
    let mut message_content = String::new();

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

                // Send tool call message to agent (which will forward to executor)
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
        let _ = tx
            .send(AgentMessage::Text {
                message_id: id.clone(),
                chunk: message_content,
                is_complete: true,
                model_name: model_name.clone(),
            })
            .await;
    }
}
