use crate::agent::llm_stream_processor::LlmStreamProcessor;
use crate::agent::tool_executor_task::ToolExecutor;
use crate::agent::{AgentMessage, UserMessage};
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::McpManager;
use crate::types::{ChatMessage, IsoString, LlmResponse};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{Level, span};

pub struct Agent<T: ModelProvider> {
    llm: Arc<T>,
    mcp: McpManager,
    context: Context,
}

impl<T: ModelProvider + 'static> Agent<T> {
    /// Create a new agent
    pub(crate) fn new(llm: T, mcp_manager: McpManager, messages: Vec<ChatMessage>) -> Self {
        let context = Context::new(messages, Vec::new());

        Self {
            llm: Arc::new(llm),
            mcp: mcp_manager,
            context,
        }
    }

    pub fn current_model_display_name(&self) -> String {
        self.llm.display_name()
    }

    /// Main agent loop - runs in a dedicated task
    pub(crate) async fn run(
        mut self,
        mut input_rx: mpsc::Receiver<UserMessage>,
        output_tx: mpsc::Sender<AgentMessage>,
    ) {
        let mut current_cancellation: Option<CancellationToken> = None;

        while let Some(message) = input_rx.recv().await {
            match message {
                UserMessage::Cancel => {
                    // Cancel current processing if any
                    if let Some(token) = current_cancellation.take() {
                        token.cancel();
                        let _ = output_tx
                            .send(AgentMessage::Cancelled {
                                message: "Processing cancelled by user".to_string(),
                            })
                            .await;
                    }
                }

                UserMessage::Text { content } => {
                    // Cancel any ongoing processing
                    if let Some(token) = current_cancellation.take() {
                        token.cancel();
                    }

                    // Create new cancellation token for this message
                    let cancellation_token = CancellationToken::new();
                    current_cancellation = Some(cancellation_token.clone());

                    // Update context with tools before processing
                    let tools = self.mcp.tool_definitions();
                    self.context.set_tools(tools);

                    self.context.add_message(ChatMessage::User {
                        content,
                        timestamp: IsoString::now(),
                    });

                    // Run the agentic loop directly
                    self.run_agentic_loop(&output_tx, cancellation_token).await;
                }
            }
        }

        tracing::debug!("Agent task shutting down - input channel closed");
    }

    /// Run the agentic loop to completion with cancellation support
    async fn run_agentic_loop(
        &mut self,
        output_tx: &mpsc::Sender<AgentMessage>,
        cancellation_token: CancellationToken,
    ) {
        let mut tool_executor = ToolExecutor::new(self.mcp.clone());

        let span = span!(Level::DEBUG, "agent_task");
        let _guard = span.enter();

        tracing::debug!("Starting agent task main loop");

        let model_name = self.llm.display_name();

        // Main agentic loop
        loop {
            // Check for cancellation before starting next iteration
            if cancellation_token.is_cancelled() {
                tracing::debug!("Agentic loop cancelled");
                let _ = output_tx
                    .send(AgentMessage::Cancelled {
                        message: "Processing cancelled".to_string(),
                    })
                    .await;
                break;
            }

            // Track iteration completion state
            let mut llm_stream_finished = false;
            let mut final_message_content: Option<String> = None;
            let mut cancelled = false;
            let mut current_message_id: Option<String> = None;
            let mut message_content = String::new();

            let mut llm_processor =
                LlmStreamProcessor::new(self.llm.clone(), Arc::new(self.context.clone()));

            // Process LLM and tool messages until iteration complete or cancellation
            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        tracing::debug!("Iteration cancelled");
                        cancelled = true;
                        break;
                    }

                    llm_response = llm_processor.recv_response() => {
                        match llm_response {
                            Some(response) => {
                                self.handle_llm_response(
                                    response,
                                    &mut tool_executor,
                                    &model_name,
                                    &mut current_message_id,
                                    &mut message_content,
                                    output_tx
                                ).await;
                            }
                            None => {
                                llm_stream_finished = true;
                                // Send final complete message if we have one
                                if let Some(ref id) = current_message_id {
                                    final_message_content = Some(message_content.clone());
                                    let _ = output_tx.send(AgentMessage::Text {
                                        message_id: id.clone(),
                                        chunk: message_content.clone(),
                                        is_complete: true,
                                        model_name: model_name.clone(),
                                    }).await;
                                }
                            }
                        }
                    }

                    tool_result = tool_executor.recv_result() => {
                        if let Some(result) = tool_result {
                            self.handle_tool_result(result, &model_name, output_tx).await;
                        }
                    }
                }

                // Check if iteration is complete:
                // If cancelled: exit immediately (don't wait for pending tools)
                // If not cancelled: wait for LLM stream to finish AND have a complete message AND no pending tools
                if cancelled {
                    tracing::debug!("Iteration cancelled");
                    break;
                }

                if llm_stream_finished
                    && final_message_content.is_some()
                    && !tool_executor.has_pending()
                {
                    tracing::debug!("Iteration complete normally");
                    break;
                }
            }

            // Shutdown the LLM processor
            llm_processor.shutdown().await;

            // Handle cancellation
            if cancelled {
                let _ = output_tx
                    .send(AgentMessage::Cancelled {
                        message: "Processing cancelled".to_string(),
                    })
                    .await;
                break;
            }

            // Update context if LLM completed successfully
            let should_continue = if let Some(final_message) = final_message_content {
                let tool_results = tool_executor.take_results();
                self.update_context(&tool_results, &final_message);

                // Continue loop if we had tool results
                !tool_results.is_empty()
            } else {
                false
            };

            tracing::debug!(
                "Agent iteration complete, should_continue: {}",
                should_continue
            );

            if !should_continue {
                break;
            }
        }

        // Clean up tool executor
        tool_executor.shutdown().await;

        tracing::debug!("Agent task main loop exited, task ending");
        if let Err(e) = output_tx.send(AgentMessage::Done).await {
            tracing::warn!("Failed to send Done message: {:?}", e);
        }
    }

    /// Handle LLM response from the stream
    async fn handle_llm_response(
        &self,
        response: LlmResponse,
        tool_executor: &mut ToolExecutor,
        model_name: &str,
        current_message_id: &mut Option<String>,
        message_content: &mut String,
        output_tx: &mpsc::Sender<AgentMessage>,
    ) {
        use LlmResponse::*;

        match response {
            Start { message_id } => {
                *current_message_id = Some(message_id);
                message_content.clear();
            }

            Text { chunk } => {
                message_content.push_str(&chunk);
                if let Some(id) = current_message_id {
                    let _ = output_tx
                        .send(AgentMessage::Text {
                            message_id: id.clone(),
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

                // Execute the tool
                let request = crate::types::ToolCallRequest {
                    id: tool_call.id,
                    name: tool_call.name,
                    arguments: tool_call.arguments,
                };
                if let Err(e) = tool_executor.send_request(request).await {
                    tracing::warn!("Failed to send tool request: {:?}", e);
                }
            }

            Done => {
                // Stream will be marked as finished when recv returns None
            }

            Error { message } => {
                let _ = output_tx.send(AgentMessage::Error { message }).await;
            }
        }
    }

    /// Handle tool result from the executor
    async fn handle_tool_result(
        &self,
        result: ToolCallResult,
        model_name: &str,
        output_tx: &mpsc::Sender<AgentMessage>,
    ) {
        tracing::debug!(
            "Tool result received: {} -> {}",
            result.name,
            result.result.len()
        );
        tracing::trace!("Processing tool result for tool_call_id: {}", result.id);

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

    /// Update the context with results from this iteration
    fn update_context(&mut self, tool_results: &[ToolCallResult], final_message: &str) {
        let mut tool_requests = Vec::new();
        for result in tool_results {
            tool_requests.push(result.request.clone());
        }

        // Add assistant message with tool calls
        let assistant_msg = ChatMessage::Assistant {
            content: final_message.to_string(),
            timestamp: IsoString::now(),
            tool_calls: tool_requests,
        };

        self.context.add_message(assistant_msg);

        // Add tool results
        for result in tool_results {
            self.context.add_message(ChatMessage::ToolCallResult {
                tool_call_id: result.id.clone(),
                content: result.result.clone(),
                timestamp: IsoString::now(),
            });
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
