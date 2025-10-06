use crate::agent::iteration_state::AgenticIterationState;
use crate::agent::{AgentMessage, UserMessage};
use crate::llm::ModelProvider;
use crate::llm::{Context, LlmError};
use crate::mcp::mcp_task::{McpCommand, McpEvent};
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
    /// MCP event (tool results, tool changes, etc.)
    Mcp(McpEvent),
}

pub struct Agent<T: ModelProvider> {
    llm: T,
    context: Context,
    mcp_command_tx: mpsc::Sender<McpCommand>,
    mcp_event_stream: ReceiverStream<McpEvent>,
    user_message_rx: mpsc::Receiver<UserMessage>,
    agent_message_tx: mpsc::Sender<AgentMessage>,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub fn new(
        llm: T,
        context: Context,
        mcp_command_tx: mpsc::Sender<McpCommand>,
        mcp_event_stream: ReceiverStream<McpEvent>,
        user_message_rx: mpsc::Receiver<UserMessage>,
        agent_message_tx: mpsc::Sender<AgentMessage>,
    ) -> Self {
        Self {
            llm,
            context,
            mcp_command_tx,
            mcp_event_stream,
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

                    self.context.add_message(ChatMessage::User {
                        content,
                        timestamp: IsoString::now(),
                    });

                    self.run_agentic_loop(cancellation_token).await;
                }
            }
        }

        // Send shutdown command to MCP task
        let _ = self.mcp_command_tx.send(McpCommand::Shutdown).await;
        tracing::debug!("Agent task shutting down - input channel closed");
    }

    async fn run_agentic_loop(&mut self, cancellation_token: CancellationToken) {
        tracing::debug!("Starting agent task main loop");
        let model_name = self.llm.display_name();

        loop {
            let mut state = AgenticIterationState::new();
            let llm_stream = self.llm.stream_response(&self.context).map(AgentEvent::Llm);
            let mcp_stream = (&mut self.mcp_event_stream).map(AgentEvent::Mcp);

            let mut events = pin!(llm_stream.merge(mcp_stream));
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
                            &self.mcp_command_tx,
                        )
                        .await;
                    }

                    AgentEvent::Mcp(mcp_event) => {
                        Self::handle_mcp_event(
                            mcp_event,
                            &mut state,
                            &model_name,
                            &self.agent_message_tx,
                        )
                        .await;
                    }
                }

                if state.is_llm_done() && state.all_tools_complete() {
                    break;
                }
            }

            tracing::debug!("Event stream complete");

            if let Some(id) = state.current_message_id() {
                let _ = self
                    .agent_message_tx
                    .send(AgentMessage::Text {
                        message_id: id.to_string(),
                        chunk: String::new(), // Empty chunk for completion signal
                        is_complete: true,
                        model_name: model_name.to_string(),
                    })
                    .await;
            }

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
        mcp_command_tx: &mpsc::Sender<McpCommand>,
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
                if let Err(e) = mcp_command_tx.send(McpCommand::ExecuteTool(request)).await {
                    tracing::warn!("Failed to send tool request to MCP task: {:?}", e);
                }
            }

            Done => {
                // LLM stream complete - mark in state
                state.mark_llm_done();
            }

            Error { message } => {
                let _ = output_tx.send(AgentMessage::Error { message }).await;
            }
        }
    }

    /// Handle MCP event from the stream
    async fn handle_mcp_event(
        event: McpEvent,
        state: &mut AgenticIterationState,
        model_name: &str,
        output_tx: &mpsc::Sender<AgentMessage>,
    ) {
        match event {
            McpEvent::ToolResult(result) => {
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

            McpEvent::ToolsChanged(_tools) => {
                // TODO: Update context with new tools when needed
                tracing::debug!("MCP tools changed - dynamic updates not yet implemented");
            }

            McpEvent::Error(message) => {
                tracing::error!("MCP error: {}", message);
                let _ = output_tx.send(AgentMessage::Error { message }).await;
            }
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
