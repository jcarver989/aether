use crate::agent::AgentMessage;
use crate::agent::UserMessage;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::{ElicitationRequest, McpManager};
use crate::types::ChatMessage;
use crate::types::IsoString;
use crate::types::LlmResponse;
use crate::types::ToolCallRequest;
use async_stream::stream;
use color_eyre::Result;
use futures::Stream;
use futures::StreamExt;
use futures::pin_mut;
use std::sync::Arc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct Agent<T: ModelProvider> {
    llm: T,
    mcp_client: Arc<Mutex<McpManager>>,
    context: Context,
    current_task_token: Option<CancellationToken>,
    elicitation_receiver: Arc<Mutex<mpsc::UnboundedReceiver<ElicitationRequest>>>,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub fn new(
        llm: T,
        mcp_client: McpManager,
        messages: Vec<ChatMessage>,
        elicitation_receiver: mpsc::UnboundedReceiver<ElicitationRequest>,
    ) -> Self {
        Self {
            llm: llm,
            mcp_client: Arc::new(Mutex::new(mcp_client)),
            context: Context::new(
                messages,
                Vec::new(), // populated when tools are discovered
            ),
            current_task_token: None,
            elicitation_receiver: Arc::new(Mutex::new(elicitation_receiver)),
        }
    }

    pub async fn current_model_display_name(&self) -> String {
        self.llm.display_name()
    }

    pub async fn send(
        &mut self,
        message: UserMessage,
    ) -> (impl Stream<Item = AgentMessage>, CancellationToken) {
        match message {
            UserMessage::Text { content } => {
                if let Some(token) = &self.current_task_token {
                    token.cancel();
                }

                let cancellation_token = CancellationToken::new();
                self.current_task_token = Some(cancellation_token.clone());
                self.context.add_message(ChatMessage::User {
                    content,
                    timestamp: IsoString::now(),
                });

                let stream = self.process_user_message().await;
                (stream, cancellation_token)
            }
        }
    }

    async fn update_tools(&mut self) -> Result<()> {
        let mut mcp = self.mcp_client.lock().await;
        mcp.discover_tools().await?;
        let tools = mcp.get_tool_definitions();
        self.context.set_tools(tools);
        Ok(())
    }

    async fn process_user_message(&mut self) -> impl Stream<Item = AgentMessage> {
        stream! {
            match self.update_tools().await {
                Ok(_) => {}
                Err(e) => {
                    yield AgentMessage::Error { message: format!("Error fetching tools: {:?}", e) };
                    return;
                }
            };

            let mut tool_collector = ToolResultsCollector::new();

            // Main "agentic" loop.
            // Each iteration of the outer loop procesess 1 LLM call
            // Each iteration of the inner loop processes 1 streaming "event" chunk from the LLM's response
            let max_iterations = 10_000;
            let mut current_iteration = 1;

            loop {
                tracing::debug!("Starting agent loop iteration {}", current_iteration);

                if current_iteration >= max_iterations {
                    tracing::error!("Max iterations reached: {}", max_iterations);
                    yield AgentMessage::Error { message: "Max iterations reached".to_string() };
                    return;
                }

                tracing::debug!("Getting LLM response stream for iteration {}", current_iteration);
                let response_stream = self.llm.stream_response(&self.context);
                let model_name = self.llm.display_name();

                pin_mut!(response_stream);

                let mut current_message_id: Option<String> = None;
                let mut message_content = String::new();

                loop {
                    if let Some(event) = response_stream.next().await {
                        use LlmResponse::*;
                        match event {
                            Ok(Start { message_id}) => {
                                current_message_id = Some(message_id);
                            }

                            Ok(Text { chunk}) => {
                                message_content.push_str(&chunk);
                                if let Some(ref id) = current_message_id {
                                    yield AgentMessage::Text {
                                        message_id: id.clone(),
                                        chunk,
                                        is_complete: false,
                                        model_name: model_name.clone()
                                    };
                                }
                            }

                            Ok(ToolRequestStart { id, name}) => {
                                yield AgentMessage::ToolCall {
                                    tool_call_id: id,
                                    name,
                                    arguments: None,
                                    result: None,
                                    is_complete: false,
                                    model_name: model_name.clone()
                                };
                            }

                            Ok(ToolRequestArg { id, chunk}) => {
                                yield AgentMessage::ToolCall {
                                    tool_call_id: id,
                                    name: String::new(),
                                    arguments: Some(chunk.to_string()),
                                    result: None,
                                    is_complete: false,
                                    model_name: model_name.clone()
                                };
                            }

                            Ok(ToolRequestComplete { tool_call}) => {
                                tracing::debug!("Tool request completed: {} ({})", tool_call.name, tool_call.id);
                                let tool_tx = tool_collector.start_tool_request(tool_call.clone());
                                let handle = Self::execute_tool(self.mcp_client.clone(), tool_call.clone(), tool_tx);
                                tool_collector.add_tool_handle(handle);
                            }

                            Ok(Done) => {
                                tracing::debug!("LLM response Done. Tool requests: {}", tool_collector.requests.len());
                                break;
                            }

                            Ok(Error { message }) => {
                                yield AgentMessage::Error { message: message.to_string() };
                                return;
                            }

                            Err(e) => {
                                yield AgentMessage::Error { message: e.to_string() };
                                return;
                            }
                        }
                    } else {
                        break;
                    }
                }

                self.context.add_message(ChatMessage::Assistant {
                    content: message_content.clone(),
                    timestamp: IsoString::now(),
                    tool_calls: tool_collector.requests.clone()
                });

                if let Some(ref id) = current_message_id {
                    yield AgentMessage::Text {
                        message_id: id.clone(),
                        chunk: String::new(),
                        is_complete: true,
                        model_name: model_name.clone()
                    };
                }

                if tool_collector.requests.is_empty() {
                    tracing::debug!("No tool requests, terminating agent loop");
                    return;
                }

                if !tool_collector.requests.is_empty() {
                    tracing::debug!("Waiting for {} tool results after stream completion...", tool_collector.requests.len());
                    let tool_results = tool_collector.wait_for_all_results().await;

                    for result in tool_results {
                        tracing::debug!("Tool result received: {} -> {}", result.name, result.result.len());

                        self.context.add_message(ChatMessage::ToolCallResult {
                            tool_call_id: result.id.clone(),
                            content: result.result.clone(),
                            timestamp: IsoString::now()
                        });

                        yield AgentMessage::ToolCall {
                            tool_call_id: result.id.clone(),
                            name: result.name.clone(),
                            arguments: Some(result.arguments.clone()),
                            result: Some(result.result.clone()),
                            is_complete: true,
                            model_name: model_name.clone(),
                        };
                    }

                    tool_collector = ToolResultsCollector::new();
                }

                current_iteration += 1;
            }
        }
    }

    fn execute_tool(
        mcp_client: Arc<Mutex<McpManager>>,
        request: ToolCallRequest,
        rx: mpsc::UnboundedSender<ToolCallResult>,
    ) -> JoinHandle<Result<(), SendError<ToolCallResult>>> {
        tokio::spawn(async move {
            let result_str = match serde_json::from_str(&request.arguments) {
                Ok(args) => {
                    let mcp_client_guard = mcp_client.lock().await;
                    match mcp_client_guard.execute_tool(&request.name, args).await {
                        Ok(result) => result.to_string(),
                        Err(e) => {
                            format!("Tool execution failed: {}", e)
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Invalid tool arguments for {}: {}", request.name, e);
                    format!("Invalid tool arguments: {}", e)
                }
            };

            rx.send(ToolCallResult {
                id: request.id.clone(),
                name: request.name.clone(),
                arguments: request.arguments,
                result: result_str,
            })
        })
    }
}

struct ToolResultsCollector {
    requests: Vec<ToolCallRequest>,
    tool_result_rx: mpsc::UnboundedReceiver<ToolCallResult>,
    tool_result_tx: mpsc::UnboundedSender<ToolCallResult>,
    completed_results: Vec<ToolCallResult>,
    tool_handles: Vec<JoinHandle<Result<(), SendError<ToolCallResult>>>>,
}

impl ToolResultsCollector {
    pub fn new() -> Self {
        let (tool_result_tx, tool_result_rx) = mpsc::unbounded_channel();
        Self {
            requests: Vec::new(),
            tool_result_rx,
            tool_result_tx,
            completed_results: Vec::new(),
            tool_handles: Vec::new(),
        }
    }

    pub fn start_tool_request(
        &mut self,
        request: ToolCallRequest,
    ) -> mpsc::UnboundedSender<ToolCallResult> {
        self.requests.push(request);
        self.tool_result_tx.clone()
    }

    pub fn add_tool_handle(&mut self, handle: JoinHandle<Result<(), SendError<ToolCallResult>>>) {
        self.tool_handles.push(handle);
    }

    pub async fn wait_for_all_results(&mut self) -> Vec<ToolCallResult> {
        let handles = std::mem::take(&mut self.tool_handles);
        tracing::debug!("Waiting for {} tool handles to complete", handles.len());
        let _ = futures::future::join_all(handles).await;

        while let Ok(result) = self.tool_result_rx.try_recv() {
            self.completed_results.push(result);
        }

        self.completed_results.clone()
    }
}

#[derive(Clone)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: String,
}
