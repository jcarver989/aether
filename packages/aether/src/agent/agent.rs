use crate::agent::iteration_state::AgenticIterationState;
use crate::agent::tool_executor_task::ToolExecutor;
use crate::agent::{AgentMessage, UserMessage};
use crate::llm::ModelProvider;
use crate::llm::{Context, LlmError};
use crate::mcp::McpManager;
use crate::types::{ChatMessage, IsoString, LlmResponse, ToolCallRequest};
use std::pin::pin;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

/// Unified event type for the agent event loop
#[derive(Debug)]
enum AgentEvent {
    /// LLM response event (success or error)
    Llm(Result<LlmResponse, LlmError>),
    /// Tool execution result
    ToolResult(ToolCallResult),
}

pub struct Agent<T: ModelProvider> {
    llm: T,
    mcp: McpManager,
    context: Context,
    tool_executor: ToolExecutor,
    tool_result_stream: ReceiverStream<ToolCallResult>,
    user_message_rx: mpsc::Receiver<UserMessage>,
    agent_message_tx: mpsc::Sender<AgentMessage>,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub fn new(
        llm: T,
        mcp_manager: McpManager,
        messages: Vec<ChatMessage>,
        user_message_rx: mpsc::Receiver<UserMessage>,
        agent_message_tx: mpsc::Sender<AgentMessage>,
    ) -> Self {
        let context = Context::new(messages, Vec::new());
        let (tool_executor, tool_result_stream) = ToolExecutor::new(mcp_manager.clone());

        Self {
            llm,
            tool_executor,
            tool_result_stream,
            mcp: mcp_manager,
            context,
            user_message_rx,
            agent_message_tx,
        }
    }

    pub fn current_model_display_name(&self) -> String {
        self.llm.display_name()
    }

    pub async fn run(mut self) {
        let mut current_cancellation: Option<CancellationToken> = None;

        while let Some(message) = self.user_message_rx.recv().await {
            match message {
                UserMessage::Cancel => {
                    if let Some(token) = current_cancellation.take() {
                        token.cancel();
                        let _ = self
                            .agent_message_tx
                            .send(AgentMessage::Cancelled {
                                message: "Processing cancelled by user".to_string(),
                            })
                            .await;
                    }
                }

                UserMessage::Text { content } => {
                    if let Some(token) = current_cancellation.take() {
                        token.cancel();
                    }

                    let cancellation_token = CancellationToken::new();
                    current_cancellation = Some(cancellation_token.clone());

                    let tools = self.mcp.tool_definitions();
                    self.context.set_tools(tools);

                    self.context.add_message(ChatMessage::User {
                        content,
                        timestamp: IsoString::now(),
                    });

                    self.run_agentic_loop(cancellation_token).await;
                }
            }
        }

        self.tool_executor.shutdown().await;
        tracing::debug!("Agent task shutting down - input channel closed");
    }

    async fn run_agentic_loop(&mut self, cancellation_token: CancellationToken) {
        tracing::debug!("Starting agent task main loop");
        let model_name = self.llm.display_name();

        loop {
            let mut state = AgenticIterationState::new();
            let llm_stream = self.llm.stream_response(&self.context).map(AgentEvent::Llm);
            let tool_stream = (&mut self.tool_result_stream).map(AgentEvent::ToolResult);

            let mut events = pin!(llm_stream.merge(tool_stream));
            while let Some(event) = events.next().await {
                if cancellation_token.is_cancelled() {
                    Self::handle_cancellation(&self.agent_message_tx).await;
                    return; // Exit entire agentic loop
                }

                match event {
                    AgentEvent::Llm(result) => {
                        Self::handle_llm_response(
                            result,
                            &mut state,
                            &model_name,
                            &self.agent_message_tx,
                            &self.tool_executor,
                        )
                        .await;
                    }

                    AgentEvent::ToolResult(result) => {
                        Self::handle_tool_result(
                            result,
                            &mut state,
                            &model_name,
                            &self.agent_message_tx,
                        )
                        .await;
                    }
                }
            }

            tracing::debug!("Event stream complete");

            // Stream ended - send final complete message if we have accumulated content
            if let Some(id) = state.current_message_id() {
                let _ = self
                    .agent_message_tx
                    .send(AgentMessage::Text {
                        message_id: id.to_string(),
                        chunk: state.message_content().to_string(),
                        is_complete: true,
                        model_name: model_name.to_string(),
                    })
                    .await;
            }

            // Update context and determine if we should continue
            let should_continue = state.should_continue_loop();

            if state.final_message_content().is_some() {
                let messages = state.into_context_messages();
                for message in messages {
                    self.context.add_message(message);
                }
            }

            tracing::debug!(
                "Agent iteration complete, should_continue: {}",
                should_continue
            );

            if !should_continue {
                break;
            }
        }

        tracing::debug!("Agent task main loop exited, task ending");
        if let Err(e) = self.agent_message_tx.send(AgentMessage::Done).await {
            tracing::warn!("Failed to send Done message: {:?}", e);
        }
    }

    /// Handle cancellation event
    async fn handle_cancellation(output_tx: &mpsc::Sender<AgentMessage>) {
        tracing::debug!("Iteration cancelled");
        let _ = output_tx
            .send(AgentMessage::Cancelled {
                message: "Processing cancelled".to_string(),
            })
            .await;
    }

    /// Handle LLM response from the stream
    async fn handle_llm_response(
        result: Result<LlmResponse, LlmError>,
        state: &mut AgenticIterationState,
        model_name: &str,
        output_tx: &mpsc::Sender<AgentMessage>,
        tool_executor: &ToolExecutor,
    ) {
        use LlmResponse::*;

        let response = match result {
            Ok(response) => response,
            Err(e) => {
                let _ = output_tx
                    .send(AgentMessage::Error {
                        message: e.to_string(),
                    })
                    .await;
                return;
            }
        };

        match response {
            Start { message_id } => {
                state.set_message_id(message_id);
            }

            Text { chunk } => {
                state.append_content(&chunk);
                if let Some(id) = state.current_message_id() {
                    let _ = output_tx
                        .send(AgentMessage::Text {
                            message_id: id.to_string(),
                            chunk,
                            is_complete: false,
                            model_name: model_name.to_string(),
                        })
                        .await;
                }
            }

            ToolRequestStart { id, name } => {
                let _ = output_tx
                    .send(AgentMessage::ToolCall {
                        tool_call_id: id,
                        name,
                        arguments: None,
                        result: None,
                        is_complete: false,
                        model_name: model_name.to_string(),
                    })
                    .await;
            }

            ToolRequestArg { id, chunk } => {
                let _ = output_tx
                    .send(AgentMessage::ToolCall {
                        tool_call_id: id,
                        name: String::new(),
                        arguments: Some(chunk.to_string()),
                        result: None,
                        is_complete: false,
                        model_name: model_name.to_string(),
                    })
                    .await;
            }

            ToolRequestComplete { tool_call } => {
                tracing::debug!(
                    "Tool request completed: {} ({})",
                    tool_call.name,
                    tool_call.id
                );

                let request = ToolCallRequest {
                    id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                };

                // Send tool call message to UI
                let _ = output_tx
                    .send(AgentMessage::ToolCall {
                        tool_call_id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        arguments: Some(tool_call.arguments.clone()),
                        result: None,
                        is_complete: false,
                        model_name: model_name.to_string(),
                    })
                    .await;

                state.mark_tool_sent(tool_call.id);
                if let Err(e) = tool_executor.send_request(request).await {
                    tracing::warn!("Failed to send tool request: {:?}", e);
                }
            }

            Done => {
                // No-op: stream completion is handled after the event loop
            }

            Error { message } => {
                let _ = output_tx.send(AgentMessage::Error { message }).await;
            }
        }
    }

    /// Handle tool result from the executor
    async fn handle_tool_result(
        result: ToolCallResult,
        state: &mut AgenticIterationState,
        model_name: &str,
        output_tx: &mpsc::Sender<AgentMessage>,
    ) {
        tracing::debug!(
            "Tool result received: {} -> {}",
            result.name,
            result.result.len()
        );
        tracing::trace!("Processing tool result for tool_call_id: {}", result.id);

        // Mark tool as complete in state
        state.mark_tool_complete(result.clone());

        // Send completion message
        let msg = AgentMessage::ToolCall {
            tool_call_id: result.id.clone(),
            name: result.name.clone(),
            arguments: Some(result.arguments.clone()),
            result: Some(result.result.clone()),
            is_complete: true,
            model_name: model_name.to_string(),
        };

        if let Err(e) = output_tx.send(msg).await {
            tracing::warn!("Failed to send ToolCall completion message: {:?}", e);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: String,
    pub request: crate::types::ToolCallRequest,
}
