use crate::agent::AgentMessage;
use crate::agent::elicitation_task::ElicitationTask;
use crate::agent::tool_execution_task::ToolExecutionTask;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager};
use crate::types::ToolCallRequest;
use crate::types::{ChatMessage, IsoString, LlmResponse};
use futures::StreamExt;
use futures::pin_mut;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument};

struct IterationState {
    current_message_id: Option<String>,
    accumulated_content: String,
    has_tool_calls: bool,
    tool_call_requests: Vec<ToolCallRequest>,
    model_name: String,
    tool_result_receivers: Vec<oneshot::Receiver<(String, String)>>, // (tool_call_id, result)
}

enum IterationResult {
    Continue,
    Done,
    Error,
}

impl IterationState {
    fn new(model_name: String) -> Self {
        Self {
            current_message_id: None,
            accumulated_content: String::new(),
            has_tool_calls: false,
            tool_call_requests: Vec::new(),
            model_name,
            tool_result_receivers: Vec::new(),
        }
    }
}

pub struct AgentTask<T: ModelProvider> {
    cancellation_token: CancellationToken,
    context: Arc<Mutex<Context>>,
    mcp_client: Arc<Mutex<McpManager>>,
    llm: Arc<Mutex<T>>,
    elicitation_receiver: Arc<Mutex<mpsc::UnboundedReceiver<ElicitationRequest>>>,
    tx: mpsc::Sender<AgentMessage>,
}

impl<T: ModelProvider> AgentTask<T> {
    pub fn new(
        cancellation_token: CancellationToken,
        context: Arc<Mutex<Context>>,
        mcp_client: Arc<Mutex<McpManager>>,
        llm: Arc<Mutex<T>>,
        elicitation_receiver: Arc<Mutex<mpsc::UnboundedReceiver<ElicitationRequest>>>,
        tx: mpsc::Sender<AgentMessage>,
    ) -> Self {
        Self {
            cancellation_token,
            context,
            mcp_client,
            llm,
            elicitation_receiver,
            tx,
        }
    }

    #[instrument(skip(self))]
    pub async fn run(self) {
        info!("AgentTask started");
        const MAX_ITERATIONS: usize = 10_000;
        let mut n_iterations = 0;

        if self.cancellation_token.is_cancelled() {
            let _ = self
                .tx
                .send(AgentMessage::Cancelled {
                    message: "Operation was cancelled".to_string(),
                })
                .await;
            return;
        }

        self.refresh_tools().await;

        // Main agent loop
        loop {
            debug!("Starting iteration {}", n_iterations);
            if self.cancellation_token.is_cancelled() {
                let _ = self
                    .tx
                    .send(AgentMessage::Cancelled {
                        message: "Operation was cancelled during agent loop".to_string(),
                    })
                    .await;
                return;
            }

            if n_iterations >= MAX_ITERATIONS {
                let _ = self
                    .tx
                    .send(AgentMessage::Error {
                        message: "Maximum recursion depth reached".to_string(),
                    })
                    .await;
                break;
            }

            while let Ok(elicitation_request) = self.elicitation_receiver.lock().await.try_recv() {
                self.handle_elicitation_request(elicitation_request);
            }

            debug!("Creating LLM stream for iteration {}", n_iterations);
            let (llm_stream, model_name) = {
                let (context_guard, llm_guard) = (self.context.lock().await, self.llm.lock().await);
                let llm_stream = llm_guard.stream_response(&context_guard);
                let model_name = llm_guard.display_name();
                (llm_stream, model_name)
            };
            debug!("LLM stream created, model: {}", model_name);

            let mut state = IterationState::new(model_name.clone());
            pin_mut!(llm_stream);

            // Main event loop
            debug!("Entering event loop for iteration {}", n_iterations);
            loop {
                tokio::select! {
                    llm_event = llm_stream.next() => {
                        if self.cancellation_token.is_cancelled() {
                            let _ = self.tx.send(AgentMessage::Cancelled {
                                message: "Operation was cancelled".to_string(),
                            }).await;
                            return;
                        }

                        match llm_event {
                            Some(Ok(event)) => {
                                match self.handle_llm_event(event, &mut state).await {
                                    IterationResult::Continue => {}
                                    IterationResult::Done => {
                                        if state.has_tool_calls {
                                            debug!("Iteration {} done with tool calls, continuing to next iteration", n_iterations);
                                            n_iterations += 1;
                                            break; // Break inner loop to continue outer loop
                                        } else {
                                            info!("Iteration {} done with no tool calls, finishing", n_iterations);
                                            return; // No tool calls, we're done
                                        }
                                    }
                                    IterationResult::Error => {
                                        error!("Error in iteration {}, returning", n_iterations);
                                        return;
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
                                // Stream ended unexpectedly
                                let _ = self.tx.send(AgentMessage::Error {
                                    message: "LLM stream ended unexpectedly".to_string(),
                                }).await;
                                return;
                            }
                        }
                    }

                    // Check for elicitation requests (non-blocking)
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(1)) => {
                        while let Ok(elicitation_request) = self.elicitation_receiver.lock().await.try_recv() {
                            self.handle_elicitation_request(elicitation_request);
                        }
                    }
                }
            }
        }
    }

    async fn refresh_tools(&self) -> () {
        let mut mcp_client_guard = self.mcp_client.lock().await;
        let mut context_guard = self.context.lock().await;
        match mcp_client_guard.discover_tools().await {
            Ok(_) => {
                context_guard.tools = mcp_client_guard.get_tool_definitions();
            }
            Err(e) => {
                let _ = self
                    .tx
                    .send(AgentMessage::Error {
                        message: format!("Failed to discover tools: {}", e),
                    })
                    .await;
                return;
            }
        }
    }

    fn handle_elicitation_request(&self, elicitation_request: ElicitationRequest) {
        let task = ElicitationTask::new(self.tx.clone(), elicitation_request);
        tokio::spawn(task.run());
    }

    #[instrument(skip(self, state))]
    async fn handle_llm_event(
        &self,
        event: LlmResponse,
        state: &mut IterationState,
    ) -> IterationResult {
        use LlmResponse::*;
        match event {
            Start { message_id } => {
                state.current_message_id = Some(message_id);
                IterationResult::Continue
            }
            Text { chunk } => {
                state.accumulated_content.push_str(&chunk);

                if let Some(message_id) = &state.current_message_id {
                    let _ = self
                        .tx
                        .send(AgentMessage::Text {
                            message_id: message_id.clone(),
                            chunk,
                            is_complete: false,
                            model_name: state.model_name.clone(),
                        })
                        .await;
                }
                IterationResult::Continue
            }

            ToolRequestStart { id, name } => {
                let _ = self
                    .tx
                    .send(AgentMessage::ToolCall {
                        tool_call_id: id,
                        name,
                        arguments: None,
                        result: None,
                        is_complete: false,
                        model_name: state.model_name.clone(),
                    })
                    .await;
                IterationResult::Continue
            }

            ToolRequestArg { id, chunk } => {
                let _ = self
                    .tx
                    .send(AgentMessage::ToolCall {
                        tool_call_id: id,
                        name: String::new(),
                        arguments: Some(chunk),
                        result: None,
                        is_complete: false,
                        model_name: state.model_name.clone(),
                    })
                    .await;
                IterationResult::Continue
            }

            ToolRequestComplete { tool_call } => {
                state.tool_call_requests.push(tool_call.clone());
                state.has_tool_calls = true;

                // Create oneshot channel for tool result
                let (result_sender, result_receiver) = oneshot::channel();
                state.tool_result_receivers.push(result_receiver);

                let task = ToolExecutionTask::new(
                    self.mcp_client.clone(),
                    self.tx.clone(),
                    tool_call,
                    state.model_name.clone(),
                    result_sender,
                );

                // Spawn tool execution in parallel
                tokio::spawn(task.run());
                IterationResult::Continue
            }

            Done => {
                if let Some(message_id) = &state.current_message_id {
                    let _ = self
                        .tx
                        .send(AgentMessage::Text {
                            message_id: message_id.clone(),
                            chunk: String::new(),
                            is_complete: true,
                            model_name: state.model_name.clone(),
                        })
                        .await;
                }

                // Wait for all tool executions to complete before proceeding
                let mut tool_results = Vec::new();
                if state.has_tool_calls {
                    let receivers = std::mem::take(&mut state.tool_result_receivers);
                    for receiver in receivers {
                        if let Ok((tool_call_id, result)) = receiver.await {
                            tool_results.push((tool_call_id, result));
                        }
                    }
                }

                // Add messages to context in the correct order
                {
                    let mut context_guard = self.context.lock().await;

                    // First, add the assistant message with tool calls
                    if state.has_tool_calls {
                        context_guard.messages.push(ChatMessage::Assistant {
                            content: state.accumulated_content.clone(),
                            timestamp: IsoString::now(),
                            tool_calls: state.tool_call_requests.clone(),
                        });
                    } else {
                        // If no tool calls, add a regular assistant message
                        context_guard.messages.push(ChatMessage::Assistant {
                            content: state.accumulated_content.clone(),
                            timestamp: IsoString::now(),
                            tool_calls: Vec::new(),
                        });
                    }

                    // Then, add all tool results in the order they were requested
                    for tool_call in &state.tool_call_requests {
                        if let Some((_, result)) = tool_results.iter().find(|(id, _)| id == &tool_call.id) {
                            context_guard.messages.push(ChatMessage::ToolCallResult {
                                tool_call_id: tool_call.id.clone(),
                                content: result.clone(),
                                timestamp: IsoString::now(),
                            });
                        }
                    }
                }

                IterationResult::Done
            }
            Error { message } => {
                let _ = self.tx.send(AgentMessage::Error { message }).await;
                IterationResult::Error
            }
        }
    }
}
