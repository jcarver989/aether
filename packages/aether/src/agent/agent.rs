use crate::agent::middleware::{AgentEvent, Middleware, MiddlewareAction};
use crate::agent::{AgentMessage, UserMessage};
use crate::llm::StreamingModelProvider;
use crate::llm::{Context, LlmError};
use crate::mcp::run_mcp_task::{McpCommand, McpEvent};
use crate::types::{ChatMessage, IsoString, LlmResponse, ToolCallRequest};
use std::collections::HashSet;
use std::pin::pin;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

/// Internal event type for merging LLM and MCP streams
#[derive(Debug)]
enum StreamEvent {
    Llm(Result<LlmResponse, LlmError>),
    Mcp(McpEvent),
}

pub struct Agent<T: StreamingModelProvider> {
    llm: T,
    context: Context,
    mcp_command_tx: mpsc::Sender<McpCommand>,
    mcp_event_stream: ReceiverStream<McpEvent>,
    user_message_rx: mpsc::Receiver<UserMessage>,
    agent_message_tx: mpsc::Sender<AgentMessage>,
    middleware: Middleware,
}

impl<T: StreamingModelProvider + 'static> Agent<T> {
    pub fn new(
        llm: T,
        context: Context,
        mcp_command_tx: mpsc::Sender<McpCommand>,
        mcp_event_rx: mpsc::Receiver<McpEvent>,
        user_message_rx: mpsc::Receiver<UserMessage>,
        agent_message_tx: mpsc::Sender<AgentMessage>,
        middleware: Middleware,
    ) -> Self {
        Self {
            llm,
            context,
            mcp_command_tx,
            mcp_event_stream: ReceiverStream::new(mcp_event_rx),
            user_message_rx,
            agent_message_tx,
            middleware,
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
                        self.on_cancel_event().await;
                    }
                }

                UserMessage::Text { content } => {
                    if let Some(token) = current_cancellation.take() {
                        token.cancel();
                    }

                    let action = self
                        .middleware
                        .emit(AgentEvent::UserMessage {
                            content: content.clone(),
                        })
                        .await;

                    if action == MiddlewareAction::Block {
                        tracing::debug!("User message blocked by middleware");
                        let _ = self
                            .agent_message_tx
                            .send(AgentMessage::Error {
                                message: "Message blocked by middleware".to_string(),
                            })
                            .await;
                        continue;
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

        tracing::debug!("Agent task shutting down - input channel closed");
    }

    async fn run_agentic_loop(&mut self, cancellation_token: CancellationToken) {
        tracing::debug!("Starting agent task main loop");
        let model_name = self.llm.display_name();

        loop {
            let mut current_message_id: Option<String> = None;
            let mut message_content = String::new();
            let mut pending_tools: HashSet<String> = HashSet::new();
            let mut completed_tools: Vec<ToolCallResult> = Vec::new();
            let mut llm_done = false;
            let mut stream = {
                let llm_stream = self
                    .llm
                    .stream_response(&self.context)
                    .map(StreamEvent::Llm);
                let mcp_stream = (&mut self.mcp_event_stream).map(StreamEvent::Mcp);
                pin!(llm_stream.merge(mcp_stream))
            };

            while let Some(event) = stream.next().await {
                if cancellation_token.is_cancelled() {
                    self.on_cancel_event().await;
                    return;
                }

                match event {
                    StreamEvent::Llm(llm_event) => {
                        Self::on_llm_event(
                            llm_event,
                            &mut current_message_id,
                            &mut message_content,
                            &mut pending_tools,
                            &mut llm_done,
                            &model_name,
                            &self.agent_message_tx,
                            &self.mcp_command_tx,
                            &self.middleware,
                        )
                        .await;
                    }

                    StreamEvent::Mcp(mcp_event) => {
                        Self::on_mcp_event(
                            mcp_event,
                            &mut pending_tools,
                            &mut completed_tools,
                            &model_name,
                            &self.agent_message_tx,
                        )
                        .await;
                    }
                }

                if llm_done && pending_tools.is_empty() {
                    break;
                }
            }

            if let Some(ref id) = current_message_id {
                let _ = self
                    .agent_message_tx
                    .send(AgentMessage::Text {
                        message_id: id.clone(),
                        chunk: String::new(), // Empty chunk for completion signal
                        is_complete: true,
                        model_name: model_name.to_string(),
                    })
                    .await;
            }

            if current_message_id.is_some() {
                self.update_context(&message_content, &completed_tools);
            } else {
                break;
            }

            if completed_tools.is_empty() {
                break;
            }
        }

        tracing::debug!("Agent task main loop exited, task ending");
        if let Err(e) = self.agent_message_tx.send(AgentMessage::Done).await {
            tracing::warn!("Failed to send Done message: {:?}", e);
        }
    }

    async fn on_cancel_event(&self) {
        let _ = self
            .agent_message_tx
            .send(AgentMessage::Cancelled {
                message: "Processing cancelled".to_string(),
            })
            .await;
    }

    async fn on_llm_event(
        result: Result<LlmResponse, LlmError>,
        current_message_id: &mut Option<String>,
        message_content: &mut String,
        pending_tools: &mut HashSet<String>,
        llm_done: &mut bool,
        model_name: &str,
        output_tx: &mpsc::Sender<AgentMessage>,
        mcp_command_tx: &mpsc::Sender<McpCommand>,
        middleware: &Middleware,
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
                let action = middleware
                    .emit(AgentEvent::ToolCall {
                        id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        arguments: tool_call.arguments.clone(),
                    })
                    .await;

                if action == MiddlewareAction::Block {
                    tracing::debug!("Tool call '{}' blocked by middleware", tool_call.name);
                    let _ = output_tx
                        .send(AgentMessage::Error {
                            message: format!("Tool '{}' blocked by middleware", tool_call.name),
                        })
                        .await;
                    return;
                }

                pending_tools.insert(tool_call.id.clone());
                let request = ToolCallRequest {
                    id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                };

                let msg_future = output_tx.send(AgentMessage::ToolCall {
                    tool_call_id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    arguments: Some(tool_call.arguments.clone()),
                    result: None,
                    is_complete: false,
                    model_name: model_name.to_string(),
                });

                let mcp_future = mcp_command_tx.send(McpCommand::ExecuteTool(request));
                let (_, mcp_result) = tokio::join!(msg_future, mcp_future);

                if let Err(e) = mcp_result {
                    tracing::warn!("Failed to send tool request to MCP task: {:?}", e);
                }
            }

            Done => {
                *llm_done = true;
            }

            Error { message } => {
                let _ = output_tx.send(AgentMessage::Error { message }).await;
            }
        }
    }

    async fn on_mcp_event(
        event: McpEvent,
        pending_tools: &mut HashSet<String>,
        completed_tools: &mut Vec<ToolCallResult>,
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
                pending_tools.remove(&result.id);
                completed_tools.push(result.clone());

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
        }
    }

    fn update_context(&mut self, message_content: &str, completed_tools: &[ToolCallResult]) {
        let tool_requests: Vec<_> = completed_tools
            .iter()
            .map(|result| result.request.clone())
            .collect();

        self.context.add_message(ChatMessage::Assistant {
            content: message_content.to_string(),
            timestamp: IsoString::now(),
            tool_calls: tool_requests,
        });

        for result in completed_tools {
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
    pub request: ToolCallRequest,
}
