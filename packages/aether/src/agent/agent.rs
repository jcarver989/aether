use crate::agent::middleware::{AgentEvent, Middleware, MiddlewareAction};
use crate::agent::{AgentMessage, UserMessage};
use crate::llm::StreamingModelProvider;
use crate::llm::{Context, LlmError};
use crate::mcp::run_mcp_task::{McpCommand, McpEvent};
use crate::types::{ChatMessage, IsoString, LlmResponse, ToolCallRequest};
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::StreamMap;
use tokio_stream::wrappers::ReceiverStream;

/// Internal event type for merging LLM and MCP streams
#[derive(Debug)]
enum StreamEvent {
    Llm(Result<LlmResponse, LlmError>),
    Mcp(McpEvent),
    UserMessage(UserMessage),
}

type EventStream = Pin<Box<dyn Stream<Item = StreamEvent> + Send>>;

pub struct Agent<T: StreamingModelProvider> {
    llm: T,
    context: Context,
    mcp_command_tx: mpsc::Sender<McpCommand>,
    agent_message_tx: mpsc::Sender<AgentMessage>,
    streams: StreamMap<String, EventStream>,
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
        let mut streams: StreamMap<String, EventStream> = StreamMap::new();
        streams.insert(
            "user".to_string(),
            Box::pin(ReceiverStream::new(user_message_rx).map(StreamEvent::UserMessage)),
        );

        streams.insert(
            "mcp".to_string(),
            Box::pin(ReceiverStream::new(mcp_event_rx).map(StreamEvent::Mcp)),
        );

        Self {
            llm,
            context,
            mcp_command_tx,
            agent_message_tx,
            streams,
            middleware,
        }
    }

    pub fn current_model_display_name(&self) -> String {
        self.llm.display_name()
    }

    pub async fn run(mut self) {
        let mut state = IterationState::new();
        let model_name = self.llm.display_name();

        while let Some((_, event)) = self.streams.next().await {
            use UserMessage::*;
            match event {
                StreamEvent::UserMessage(Cancel) => {
                    self.on_user_cancel(&mut state).await;
                }

                StreamEvent::UserMessage(Text { content }) => {
                    self.on_user_text(content).await;
                }

                StreamEvent::Llm(llm_event) => {
                    if !state.cancelled {
                        self.on_llm_event(llm_event, &mut state).await;
                    }
                }

                StreamEvent::Mcp(mcp_event) => {
                    if !state.cancelled {
                        self.on_mcp_event(mcp_event, &mut state).await;
                    }
                }
            }

            if state.is_complete()
                && let Some(ref id) = state.current_message_id
            {
                self.update_context(&state.message_content, &state.completed_tool_calls);
                let _ = self
                    .agent_message_tx
                    .send(AgentMessage::Text {
                        message_id: id.clone(),
                        chunk: String::new(), // Empty chunk for completion signal
                        is_complete: true,
                        model_name: model_name.to_string(),
                    })
                    .await;

                let should_continue = state.has_tool_calls();
                state = IterationState::new();

                if should_continue {
                    self.start_llm_stream();
                } else {
                    if let Err(e) = self.agent_message_tx.send(AgentMessage::Done).await {
                        tracing::warn!("Failed to send Done message: {:?}", e);
                    }
                }
            }
        }

        tracing::debug!("Agent task shutting down - input channel closed");
    }

    async fn on_user_cancel(&mut self, state: &mut IterationState) {
        state.cancelled = true;
        self.streams.remove("llm");
        let _ = self
            .agent_message_tx
            .send(AgentMessage::Cancelled {
                message: "Processing cancelled".to_string(),
            })
            .await;
        let _ = self.agent_message_tx.send(AgentMessage::Done).await;
    }

    async fn on_user_text(&mut self, content: String) {
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
            return;
        }

        self.context.add_message(ChatMessage::User {
            content,
            timestamp: IsoString::now(),
        });

        self.start_llm_stream();
    }

    fn start_llm_stream(&mut self) {
        self.streams.remove("llm");

        let llm_stream = self
            .llm
            .stream_response(&self.context)
            .map(StreamEvent::Llm);

        self.streams.insert("llm".to_string(), Box::pin(llm_stream));
    }

    async fn on_llm_event(
        &mut self,
        result: Result<LlmResponse, LlmError>,
        state: &mut IterationState,
    ) {
        use LlmResponse::*;

        let response = match result {
            Ok(response) => response,
            Err(e) => {
                let _ = self
                    .agent_message_tx
                    .send(AgentMessage::Error {
                        message: e.to_string(),
                    })
                    .await;
                return;
            }
        };

        match response {
            Start { message_id } => {
                state.current_message_id = Some(message_id);
                state.message_content.clear();
            }

            Text { chunk } => {
                state.message_content.push_str(&chunk);

                if let Some(id) = &state.current_message_id {
                    let _ = self
                        .agent_message_tx
                        .send(AgentMessage::Text {
                            message_id: id.clone(),
                            chunk,
                            is_complete: false,
                            model_name: self.llm.display_name(),
                        })
                        .await;
                }
            }

            ToolRequestStart { id, name } => {
                let _ = self
                    .agent_message_tx
                    .send(AgentMessage::ToolCall {
                        tool_call_id: id,
                        name,
                        arguments: None,
                        result: None,
                        is_complete: false,
                        model_name: self.llm.display_name(),
                    })
                    .await;
            }

            ToolRequestArg { id, chunk } => {
                let _ = self
                    .agent_message_tx
                    .send(AgentMessage::ToolCall {
                        tool_call_id: id,
                        name: String::new(),
                        arguments: Some(chunk.to_string()),
                        result: None,
                        is_complete: false,
                        model_name: self.llm.display_name(),
                    })
                    .await;
            }

            ToolRequestComplete { tool_call } => {
                let action = self
                    .middleware
                    .emit(AgentEvent::ToolCall {
                        id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        arguments: tool_call.arguments.clone(),
                    })
                    .await;

                if action == MiddlewareAction::Block {
                    tracing::debug!("Tool call '{}' blocked by middleware", tool_call.name);
                    let _ = self
                        .agent_message_tx
                        .send(AgentMessage::Error {
                            message: format!("Tool '{}' blocked by middleware", tool_call.name),
                        })
                        .await;
                    return;
                }

                let request = ToolCallRequest {
                    id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                };

                state
                    .pending_tool_calls
                    .insert(tool_call.id.clone(), request.clone());

                let msg_future = self.agent_message_tx.send(AgentMessage::ToolCall {
                    tool_call_id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    arguments: Some(tool_call.arguments.clone()),
                    result: None,
                    is_complete: false,
                    model_name: self.llm.display_name(),
                });

                let mcp_future = self.mcp_command_tx.send(McpCommand::ExecuteTool(request));
                let (_, mcp_result) = tokio::join!(msg_future, mcp_future);

                if let Err(e) = mcp_result {
                    tracing::warn!("Failed to send tool request to MCP task: {:?}", e);
                }
            }

            Done => {
                state.llm_done = true;
            }

            Error { message } => {
                let _ = self
                    .agent_message_tx
                    .send(AgentMessage::Error { message })
                    .await;
            }
        }
    }

    async fn on_mcp_event(&mut self, event: McpEvent, state: &mut IterationState) {
        match event {
            McpEvent::ToolResult(result) => {
                tracing::debug!(
                    "Tool result received: {} -> {}",
                    result.name,
                    result.result.len()
                );
                tracing::trace!("Processing tool result for tool_call_id: {}", result.id);

                // Only process results for active tool requests
                if let Some(request) = state.pending_tool_calls.remove(&result.id) {
                    state.completed_tool_calls.push(ToolCallResult {
                        id: result.id.clone(),
                        name: result.name.clone(),
                        arguments: result.arguments.clone(),
                        result: result.result.clone(),
                        request,
                    });

                    let msg = AgentMessage::ToolCall {
                        tool_call_id: result.id.clone(),
                        name: result.name.clone(),
                        arguments: Some(result.arguments.clone()),
                        result: Some(result.result.clone()),
                        is_complete: true,
                        model_name: self.llm.display_name(),
                    };

                    if let Err(e) = self.agent_message_tx.send(msg).await {
                        tracing::warn!("Failed to send ToolCall completion message: {:?}", e);
                    }
                } else {
                    tracing::debug!("Ignoring stale tool result for id: {}", result.id);
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

#[derive(Debug)]
struct IterationState {
    pub current_message_id: Option<String>,
    pub message_content: String,
    pub pending_tool_calls: HashMap<String, ToolCallRequest>,
    pub completed_tool_calls: Vec<ToolCallResult>,
    pub llm_done: bool,
    pub cancelled: bool,
}

impl IterationState {
    pub fn new() -> Self {
        Self {
            current_message_id: None,
            message_content: String::new(),
            pending_tool_calls: HashMap::new(),
            completed_tool_calls: Vec::new(),
            llm_done: false,
            cancelled: false,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.llm_done && self.pending_tool_calls.is_empty()
    }

    pub fn has_tool_calls(&self) -> bool {
        !self.completed_tool_calls.is_empty()
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
