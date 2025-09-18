use crate::agent::AgentMessage;
use crate::agent::Result;
use crate::agent::UserMessage;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::McpManager;
use crate::mcp::manager::parse_namespaced_tool_name;
use crate::types::ChatMessage;
use crate::types::IsoString;
use crate::types::LlmResponse;
use crate::types::ToolCallRequest;
use crate::types::ToolDefinition;
use futures::Stream;
use futures::StreamExt;
use futures::pin_mut;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

pub struct Agent<T: ModelProvider> {
    llm: Arc<T>,
    mcp: Arc<Mutex<McpManager>>,
    context: Arc<Mutex<Context>>,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub fn new(llm: T, mcp_manager: McpManager, messages: Vec<ChatMessage>) -> Self {
        let mcp = Arc::new(Mutex::new(mcp_manager));

        Self {
            llm: Arc::new(llm),
            mcp,
            context: Arc::new(Mutex::new(Context::new(
                messages,
                Vec::new(), // populated when tools are discovered
            ))),
        }
    }

    pub async fn current_model_display_name(&self) -> String {
        self.llm.display_name()
    }

    pub async fn send(&mut self, message: UserMessage) -> impl Stream<Item = AgentMessage> {
        match message {
            UserMessage::Text { content } => {
                self.context.lock().await.add_message(ChatMessage::User {
                    content,
                    timestamp: IsoString::now(),
                });

                self.process_user_message().await
            }
        }
    }

    async fn update_tools(&mut self) -> Result<()> {
        let mut mcp = self.mcp.lock().await;
        mcp.discover_tools().await?;

        let tools = mcp
            .tools()
            .iter()
            .map(|(namespaced_tool_name, tool)| {
                let server_name = parse_namespaced_tool_name(namespaced_tool_name)
                    .map(|(server, _)| server.to_string());

                ToolDefinition {
                    name: namespaced_tool_name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.to_string(),
                    server: server_name,
                }
            })
            .collect();

        self.context.lock().await.set_tools(tools);
        Ok(())
    }

    async fn process_user_message(&mut self) -> impl Stream<Item = AgentMessage> {
        let tool_executor = ToolExecutor::new();
        let (_tool_executor_handle, tool_call_tx, mut tool_result_rx) =
            tool_executor.start(self.mcp.clone());

        let (tx, rx) = mpsc::channel::<AgentMessage>(100);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        let llm = self.llm.clone();
        let context = self.context.clone();

        // Update tools before spawning the task
        let update_tools_result = self.update_tools().await;

        let _task = tokio::spawn(async move {
            if let Err(e) = update_tools_result {
                let _ = tx
                    .send(AgentMessage::Error {
                        message: format!("Error fetching tools: {:?}", e),
                    })
                    .await;
                return;
            }

            // Main "agentic" loop.
            // Each iteration of the outer loop procesess 1 LLM call
            // Each iteration of the inner loop processes 1 streaming "event" chunk from the LLM's response
            let _max_iterations = 10_000;
            let mut _current_iteration = 1;

            loop {
                let model_name = llm.display_name();
                let (llm_tx, mut llm_rx) = mpsc::channel::<AgentMessage>(100);

                let llm_handle = tokio::spawn(Self::process_llm_stream(
                    llm.clone(),
                    context.clone(),
                    llm_tx,
                    tool_call_tx.clone(),
                ));

                let mut llm_completed = false;
                let mut tools_called = false;

                // Process LLM streaming events and tool results concurrently
                loop {
                    tokio::select! {
                        // Handle LLM streaming events
                        llm_msg = llm_rx.recv() => {
                            tracing::trace!("Received LLM message: {:?}", llm_msg.is_some());
                            match llm_msg {
                                Some(msg) => {
                                    tracing::trace!("Forwarding LLM message to output");
                                    if let Err(e) = tx.send(msg).await {
                                        tracing::warn!("Failed to send LLM message to output: {:?}", e);
                                        // Don't break - continue processing tool results
                                    }
                                }
                                None => {
                                    // LLM stream completed
                                    llm_completed = true;
                                    let pending_count = tool_executor.get_pending_count();
                                    tracing::debug!("LLM completed, pending tools: {}", pending_count);
                                    tracing::trace!("LLM stream channel closed");
                                }
                            }
                        }

                        // Handle tool execution results
                        tool_result = tool_result_rx.recv() => {
                            tracing::trace!("Tool result channel event: {:?}", tool_result.is_some());
                            match tool_result {
                                Some(result) => {
                                    tools_called = true;
                                    tracing::debug!(
                                        "Tool result received: {} -> {}",
                                        result.name,
                                        result.result.len()
                                    );
                                    tracing::trace!("Processing tool result for tool_call_id: {}", result.id);

                                    // Add tool result to context
                                    {
                                    tracing::trace!("Adding tool result to context");
                                    context
                                        .lock()
                                        .await
                                        .add_message(ChatMessage::ToolCallResult {
                                            tool_call_id: result.id.clone(),
                                            content: result.result.clone(),
                                            timestamp: IsoString::now(),
                                        });
                                    tracing::trace!("Tool result added to context");
                                    }

                                    // Yield tool call result message
                                    let msg = AgentMessage::ToolCall {
                                        tool_call_id: result.id.clone(),
                                        name: result.name.clone(),
                                        arguments: Some(result.arguments.clone()),
                                        result: Some(result.result.clone()),
                                        is_complete: true,
                                        model_name: model_name.clone(),
                                    };

                                    tracing::debug!("Sending ToolCall completion message with result: {} chars", result.result.len());
                                    if let Err(e) = tx.send(msg).await {
                                        tracing::warn!("Failed to send ToolCall completion message: {:?}", e);
                                        // Don't break - tool result was processed successfully
                                    }
                                    tracing::debug!("Successfully sent ToolCall completion message");
                                }
                                None => {
                                    // Tool result channel closed - this shouldn't happen in normal operation
                                    tracing::warn!("Tool result channel closed unexpectedly");
                                    break;
                                }
                            }
                        }
                    }

                    if llm_completed && tool_executor.get_pending_count() == 0 {
                        tracing::trace!(
                            "Breaking from select loop - LLM completed and no pending tools"
                        );
                        break;
                    }
                }

                let _ = llm_handle.await;
                _current_iteration += 1;
            }
        });
        return stream;
    }

    async fn process_llm_stream(
        llm: Arc<T>,
        context: Arc<Mutex<Context>>,
        tx: Sender<AgentMessage>,
        tool_call_tx: Sender<ToolCallRequest>,
    ) -> () {
        let response_stream = {
            let c = context.lock().await;
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

                    // Send message indicating tool execution has started (for tracking pending tools)
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
            context.lock().await.add_message(ChatMessage::Assistant {
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
    }
}

struct ToolExecutor {
    pending_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl ToolExecutor {
    pub fn new() -> Self {
        Self {
            pending_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    pub fn get_pending_count(&self) -> usize {
        self.pending_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn start(
        &self,
        mcp: Arc<Mutex<McpManager>>,
    ) -> (
        JoinHandle<()>,
        Sender<ToolCallRequest>,
        Receiver<ToolCallResult>,
    ) {
        let (tool_call_tx, mut tool_call_rx) = mpsc::channel::<ToolCallRequest>(100);
        let (tool_result_tx, tool_result_rx) = mpsc::channel::<ToolCallResult>(100);
        let pending_count = self.pending_count.clone();

        let handle = tokio::spawn(async move {
            while let Some(request) = tool_call_rx.recv().await {
                let new_pending =
                    pending_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                tracing::debug!(
                    "ToolExecutor received request: {} ({}) - pending count now: {}",
                    request.name,
                    request.id,
                    new_pending
                );

                let result_str = match serde_json::from_str(&request.arguments) {
                    Ok(args) => {
                        tracing::trace!("Executing tool {} with parsed args", request.name);
                        let mcp_client_guard = mcp.lock().await;
                        match mcp_client_guard.execute_tool(&request.name, args).await {
                            Ok(result) => {
                                tracing::trace!(
                                    "Tool {} execution successful, result length: {}",
                                    request.name,
                                    result.to_string().len()
                                );
                                result.to_string()
                            }
                            Err(e) => {
                                tracing::warn!("Tool {} execution failed: {}", request.name, e);
                                format!("Tool execution failed: {}", e)
                            }
                        }
                    }

                    Err(e) => {
                        tracing::error!("Invalid tool arguments for {}: {}", request.name, e);
                        format!("Invalid tool arguments: {}", e)
                    }
                };

                let tool_result = ToolCallResult {
                    id: request.id.clone(),
                    name: request.name.clone(),
                    arguments: request.arguments,
                    result: result_str.clone(),
                };

                tracing::trace!(
                    "Sending tool result for {} ({}) - result length: {}",
                    request.name,
                    request.id,
                    result_str.len()
                );
                match tool_result_tx.send(tool_result).await {
                    Ok(_) => {
                        tracing::trace!(
                            "Successfully sent tool result for {} ({})",
                            request.name,
                            request.id
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to send tool result for {} ({}): {:?}",
                            request.name,
                            request.id,
                            e
                        );
                    }
                }

                let new_pending =
                    pending_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) - 1;
                tracing::debug!(
                    "ToolExecutor completed {} ({}) - pending count now: {}",
                    request.name,
                    request.id,
                    new_pending
                );
            }
            tracing::trace!("ToolExecutor task ending - tool_call_rx channel closed");
        });

        (handle, tool_call_tx, tool_result_rx)
    }
}

#[derive(Clone)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: String,
}
