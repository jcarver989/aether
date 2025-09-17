use crate::agent::AgentMessage;
use crate::agent::elicitation_task::ElicitationTask;
use crate::agent::tool_execution_task::ToolExecutionTask;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager};
use crate::types::ToolCallRequest;
use crate::types::LlmResponse;
use color_eyre::Report;
use futures::Stream;
use futures::StreamExt;
use futures::future::join_all;
use futures::pin_mut;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc::error::SendError;
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

pub struct ProcessUserMessageTask<T: ModelProvider> {
    cancellation_token: CancellationToken,
    context: Arc<Mutex<Context>>,
    mcp_client: Arc<Mutex<McpManager>>,
    llm: Arc<Mutex<T>>,
    elicitation_receiver: Arc<Mutex<mpsc::UnboundedReceiver<ElicitationRequest>>>,
    tx: mpsc::Sender<AgentMessage>,
}

impl<T: ModelProvider> ProcessUserMessageTask<T> {
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
            let _ = self.send_cancelled_message().await;
            return;
        }

        self.refresh_tools().await;

        // Main agent loop
        loop {
            debug!("Starting iteration {}", n_iterations);
            if self.cancellation_token.is_cancelled() {
                let _ = self.send_cancelled_message().await;
                return;
            }

            if n_iterations >= MAX_ITERATIONS {
                let _ = self.send_max_iterations_reached_message().await;
                break;
            }

            let (response_stream, model_name) = self.stream_llm_response().await;
            let mut state = IterationState::new(model_name.clone());
            pin_mut!(response_stream);

            let mut elicitation_receiver = self.elicitation_receiver.lock().await;

            // Main event loop
            loop {
                tokio::select! {
                    llm_event = response_stream.next() => {

                        if self.cancellation_token.is_cancelled() {
                            let _ = self.send_cancelled_message().await;
                            return;
                        }

                        match llm_event {
                            Some(Ok(event)) => {
                                let message = self.handle_event(&event, &mut state).await;

                                // Send the message if one was produced
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
                                                n_iterations += 1;
                                                break; // Break inner loop to continue outer loop
                                            } else {
                                                info!("Iteration {} done with no tool calls, finishing", n_iterations);
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

                    // Handle elicitation requests as they arrive
                    elicitation_result = elicitation_receiver.recv() => {
                        match elicitation_result {
                            Some(elicitation_request) => {
                                self.process_elicitation_request(elicitation_request);
                            }
                            None => {
                                // Elicitation channel closed, this is expected during shutdown
                                debug!("Elicitation channel closed");
                            }
                        }
                    }
                }
            }
        }
    }

    async fn stream_llm_response(
        &self,
    ) -> (
        Pin<Box<dyn Stream<Item = Result<LlmResponse, Report>> + Send>>,
        String,
    ) {
        debug!("Creating LLM stream");
        // Acquire locks sequentially to prevent deadlocks
        let context = self.context.lock().await;
        let llm = self.llm.lock().await;
        let response_stream = llm.stream_response(&context);
        let model_name = llm.display_name();
        (response_stream, model_name)
    }

    async fn handle_event(
        &self,
        event: &LlmResponse,
        state: &mut IterationState,
    ) -> Option<AgentMessage> {
        use LlmResponse::*;

        match event {
            Start { message_id } => self.handle_start(state, message_id),
            Text { chunk } => self.handle_text(state, chunk).await,
            ToolRequestStart { id, name } => self.handle_tool_request_start(state, id, name).await,
            ToolRequestArg { id, chunk } => self.handle_tool_request_arg(state, id, chunk).await,
            ToolRequestComplete { tool_call } => {
                self.handle_tool_request_complete(state, tool_call).await
            }
            Done => self.handle_done(state).await,
            Error { message } => self.handle_error(message).await,
        }
    }

    async fn send_cancelled_message(&self) -> Result<(), SendError<AgentMessage>> {
        let msg = AgentMessage::Cancelled {
            message: "Operation was cancelled during agent loop".to_string(),
        };
        self.tx.send(msg).await
    }

    async fn send_max_iterations_reached_message(&self) -> Result<(), SendError<AgentMessage>> {
        let msg = AgentMessage::Error {
            message: "Maximum recursion depth reached".to_string(),
        };
        self.tx.send(msg).await
    }

    async fn refresh_tools(&self) -> () {
        // Acquire locks in consistent order: context first, then mcp_client
        let mut context_guard = self.context.lock().await;
        let mut mcp_client_guard = self.mcp_client.lock().await;
        match mcp_client_guard.discover_tools().await {
            Ok(_) => {
                context_guard.set_tools(mcp_client_guard.get_tool_definitions());
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

    fn process_elicitation_request(&self, elicitation_request: ElicitationRequest) {
        let task = ElicitationTask::new(self.tx.clone(), elicitation_request);
        tokio::spawn(task.run());
    }

    fn handle_start(&self, state: &mut IterationState, message_id: &str) -> Option<AgentMessage> {
        state.current_message_id = Some(message_id.to_string());
        None
    }

    async fn handle_text(&self, state: &mut IterationState, chunk: &str) -> Option<AgentMessage> {
        state.accumulated_content.push_str(chunk);

        if let Some(message_id) = &state.current_message_id {
            return Some(AgentMessage::Text {
                message_id: message_id.clone(),
                chunk: chunk.to_string(),
                is_complete: false,
                model_name: state.model_name.clone(),
            });
        }
        None
    }

    async fn handle_tool_request_start(
        &self,
        state: &mut IterationState,
        id: &str,
        name: &str,
    ) -> Option<AgentMessage> {
        Some(AgentMessage::ToolCall {
            tool_call_id: id.to_string(),
            name: name.to_string(),
            arguments: None,
            result: None,
            is_complete: false,
            model_name: state.model_name.clone(),
        })
    }

    async fn handle_tool_request_arg(
        &self,
        state: &mut IterationState,
        id: &str,
        chunk: &str,
    ) -> Option<AgentMessage> {
        Some(AgentMessage::ToolCall {
            tool_call_id: id.to_string(),
            name: String::new(),
            arguments: Some(chunk.to_string()),
            result: None,
            is_complete: false,
            model_name: state.model_name.clone(),
        })
    }

    async fn handle_tool_request_complete(
        &self,
        state: &mut IterationState,
        tool_call: &ToolCallRequest,
    ) -> Option<AgentMessage> {
        state.tool_call_requests.push(tool_call.clone());
        state.has_tool_calls = true;

        let (result_sender, result_receiver) = oneshot::channel();
        state.tool_result_receivers.push(result_receiver);

        let task = ToolExecutionTask::new(
            self.mcp_client.clone(),
            self.tx.clone(),
            tool_call.clone(),
            state.model_name.clone(),
            result_sender,
        );

        tokio::spawn(task.run());
        None
    }

    async fn handle_done(&self, state: &mut IterationState) -> Option<AgentMessage> {
        // Wait for all tool executions to complete in parallel
        let mut tool_results = Vec::new();
        if state.has_tool_calls {
            let receivers = std::mem::take(&mut state.tool_result_receivers);
            // Collect all results concurrently instead of sequentially
            let results = join_all(receivers).await;
            for result in results {
                if let Ok((tool_call_id, tool_result)) = result {
                    tool_results.push((tool_call_id, tool_result));
                }
            }
        }

        {
            let mut context_guard = self.context.lock().await;

            if state.has_tool_calls {
                context_guard.add_assistant_message_with_tools(
                    state.accumulated_content.clone(),
                    state.tool_call_requests.clone(),
                );
            } else {
                context_guard.add_assistant_message(state.accumulated_content.clone());
            }

            // Then, add all tool results in the order they were requested
            for tool_call in &state.tool_call_requests {
                if let Some((_, result)) = tool_results.iter().find(|(id, _)| id == &tool_call.id) {
                    context_guard.add_tool_call_result(tool_call.id.clone(), result.clone());
                }
            }
        }

        if let Some(message_id) = &state.current_message_id {
            Some(AgentMessage::Text {
                message_id: message_id.clone(),
                chunk: String::new(),
                is_complete: true,
                model_name: state.model_name.clone(),
            })
        } else {
            None
        }
    }

    async fn handle_error(&self, message: &str) -> Option<AgentMessage> {
        Some(AgentMessage::Error {
            message: message.to_string(),
        })
    }
}
