use crate::agent::AgentMessage;
use crate::agent::tool_execution_task::ToolExecutionTask;
use crate::llm::{Context, ModelProvider};
use crate::mcp::McpManager;
use crate::types::{LlmResponse, ToolCallRequest};
use futures::{StreamExt, future::join_all, pin_mut};
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument};

pub struct ProcessMessageTask<T: ModelProvider> {
    cancellation_token: CancellationToken,
    context: Arc<Mutex<Context>>,
    mcp_client: Arc<Mutex<McpManager>>,
    llm: Arc<Mutex<T>>,
    tx: mpsc::Sender<AgentMessage>,
}

impl<T: ModelProvider> ProcessMessageTask<T> {
    pub fn new(
        cancellation_token: CancellationToken,
        context: Arc<Mutex<Context>>,
        mcp_client: Arc<Mutex<McpManager>>,
        llm: Arc<Mutex<T>>,
        tx: mpsc::Sender<AgentMessage>,
    ) -> Self {
        Self {
            cancellation_token,
            context,
            mcp_client,
            llm,
            tx,
        }
    }

    #[instrument(skip_all)]
    pub async fn run(self) {
        info!("Processing message");
        const MAX_ITERATIONS: usize = 10_000;
        let mut n_iterations = 0;

        // Main agent loop
        loop {
            debug!("Starting iteration {}", n_iterations);
            if self.cancellation_token.is_cancelled() {
                let _ = self.tx
                    .send(AgentMessage::Cancelled {
                        message: "Operation was cancelled during processing".to_string(),
                    })
                    .await;
                return;
            }

            if n_iterations >= MAX_ITERATIONS {
                let _ = self.tx
                    .send(AgentMessage::Error {
                        message: "Maximum recursion depth reached".to_string(),
                    })
                    .await;
                break;
            }

            // Get fresh response stream for this iteration
            let llm_guard = self.llm.lock().await;
            let model_name = llm_guard.display_name();
            let context_guard = self.context.lock().await;
            let response_stream = llm_guard.stream_response(&context_guard);
            drop(context_guard);
            drop(llm_guard); // Release the lock

            let mut state = IterationState::new(model_name.clone());
            pin_mut!(response_stream);

            // Main event loop for this iteration
            loop {
                tokio::select! {
                    llm_event = response_stream.next() => {
                        if self.cancellation_token.is_cancelled() {
                            let _ = self.tx.send(AgentMessage::Cancelled {
                                message: "Operation was cancelled during LLM processing".to_string(),
                            }).await;
                            return;
                        }

                        match llm_event {
                            Some(Ok(event)) => {
                                let message = Self::handle_event(&event, &mut state, &self.mcp_client, &self.tx, &model_name).await;

                                if let Some(agent_message) = message {
                                    match &agent_message {
                                        AgentMessage::Error { .. } => {
                                            let _ = self.tx.send(agent_message).await;
                                            error!("Error in iteration {}, returning", n_iterations);
                                            return;
                                        }
                                        AgentMessage::Text { is_complete: true, .. } => {
                                            let _ = self.tx.send(agent_message).await;
                                            if state.has_tool_calls {
                                                debug!("Iteration {} done with tool calls, continuing to next iteration", n_iterations);

                                                // Wait for all tool executions to complete and update context
                                                Self::finalize_iteration(&self.context, &mut state).await;

                                                n_iterations += 1;
                                                break; // Break inner loop to continue outer loop
                                            } else {
                                                info!("Iteration {} done with no tool calls, finishing", n_iterations);

                                                // Update context with final message
                                                self.context.lock().await.add_assistant_message(state.accumulated_content.clone(), Vec::new());

                                                return; // No tool calls, we're done
                                            }
                                        }
                                        _ => {
                                            let _ = self.tx.send(agent_message).await;
                                        }
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                let _ = self.tx.send(AgentMessage::Error {
                                    message: e.to_string(),
                                }).await;
                                return;
                            }
                            None => {
                                let _ = self.tx.send(AgentMessage::Error {
                                    message: "LLM stream ended unexpectedly".to_string(),
                                }).await;
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn handle_event(
        event: &LlmResponse,
        state: &mut IterationState,
        mcp_client: &Arc<Mutex<McpManager>>,
        tx: &mpsc::Sender<AgentMessage>,
        model_name: &str,
    ) -> Option<AgentMessage> {
        use LlmResponse::*;

        match event {
            Start { message_id } => {
                state.current_message_id = Some(message_id.clone());
                None
            }
            Text { chunk } => {
                state.accumulated_content.push_str(chunk);
                if let Some(message_id) = &state.current_message_id {
                    Some(AgentMessage::Text {
                        message_id: message_id.clone(),
                        chunk: chunk.clone(),
                        is_complete: false,
                        model_name: model_name.to_string(),
                    })
                } else {
                    None
                }
            }
            ToolRequestStart { id, name } => Some(AgentMessage::ToolCall {
                tool_call_id: id.clone(),
                name: name.clone(),
                arguments: None,
                result: None,
                is_complete: false,
                model_name: model_name.to_string(),
            }),
            ToolRequestArg { id, chunk } => Some(AgentMessage::ToolCall {
                tool_call_id: id.clone(),
                name: String::new(),
                arguments: Some(chunk.clone()),
                result: None,
                is_complete: false,
                model_name: model_name.to_string(),
            }),
            ToolRequestComplete { tool_call } => {
                state.tool_call_requests.push(tool_call.clone());
                state.has_tool_calls = true;

                let (result_sender, result_receiver) = oneshot::channel();
                state.tool_result_receivers.push(result_receiver);

                let task = ToolExecutionTask::new(
                    mcp_client.clone(),
                    tx.clone(),
                    tool_call.clone(),
                    model_name.to_string(),
                    result_sender,
                );

                tokio::spawn(task.run());
                None
            }
            Done => {
                if let Some(message_id) = &state.current_message_id {
                    Some(AgentMessage::Text {
                        message_id: message_id.clone(),
                        chunk: String::new(),
                        is_complete: true,
                        model_name: model_name.to_string(),
                    })
                } else {
                    None
                }
            }
            Error { message } => Some(AgentMessage::Error {
                message: message.clone(),
            }),
        }
    }

    async fn finalize_iteration(context: &Arc<Mutex<Context>>, state: &mut IterationState) {
        // Wait for all tool executions to complete in parallel
        let mut tool_results = Vec::new();
        if state.has_tool_calls {
            let receivers = std::mem::take(&mut state.tool_result_receivers);
            let results = join_all(receivers).await;
            for result in results {
                if let Ok((tool_call_id, tool_result)) = result {
                    tool_results.push((tool_call_id, tool_result));
                }
            }
        }

        // Add assistant message with tool calls
        let mut context_guard = context.lock().await;
        context_guard.add_assistant_message(
            state.accumulated_content.clone(),
            state.tool_call_requests.clone(),
        );

        // Add all tool results in the order they were requested
        for tool_call in &state.tool_call_requests {
            if let Some((_, result)) = tool_results.iter().find(|(id, _)| id == &tool_call.id) {
                context_guard.add_tool_call_result(tool_call.id.clone(), result.clone());
            }
        }
    }
}

struct IterationState {
    current_message_id: Option<String>,
    accumulated_content: String,
    has_tool_calls: bool,
    tool_call_requests: Vec<ToolCallRequest>,
    tool_result_receivers: Vec<oneshot::Receiver<(String, String)>>, // (tool_call_id, result)
}

impl IterationState {
    fn new(_model_name: String) -> Self {
        Self {
            current_message_id: None,
            accumulated_content: String::new(),
            has_tool_calls: false,
            tool_call_requests: Vec::new(),
            tool_result_receivers: Vec::new(),
        }
    }
}