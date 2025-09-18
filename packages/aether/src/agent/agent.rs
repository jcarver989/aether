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
use async_stream::stream;
use futures::Stream;
use futures::StreamExt;
use futures::pin_mut;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::SendError;
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
        let (tool_executor_handle, tool_call_tx, mut tool_result_rx) =
            ToolExecutor::start(self.mcp.clone());

        let (tx, rx) = mpsc::channel::<AgentMessage>(100);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        let llm = self.llm.clone();
        let context = self.context.clone();
        let mcp = self.mcp.clone();

        let task = tokio::spawn(async move {
            let update_tools_result = self.update_tools().await;

            if let Err(e) = update_tools_result {
                tx.send(AgentMessage::Error {
                    message: format!("Error fetching tools: {:?}", e),
                });
            }

            // Main "agentic" loop.
            // Each iteration of the outer loop procesess 1 LLM call
            // Each iteration of the inner loop processes 1 streaming "event" chunk from the LLM's response
            let max_iterations = 10_000;
            let mut current_iteration = 1;

            loop {
                let model_name = self.llm.display_name();
                let (llm_tx, llm_rx) = mpsc::channel::<AgentMessage>(100);

                let llm_future = Self::process_llm_stream(&llm, context, llm_tx, tool_call_tx);

             

                    for result in tool_results {
                        tracing::debug!(
                            "Tool result received: {} -> {}",
                            result.name,
                            result.result.len()
                        );

                        self.context
                            .lock()
                            .await
                            .add_message(ChatMessage::ToolCallResult {
                                tool_call_id: result.id.clone(),
                                content: result.result.clone(),
                                timestamp: IsoString::now(),
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
                }

                current_iteration += 1;
            }
        });
        return stream;
    }

    async fn process_llm_stream(
        llm: &T,
        context: Arc<Mutex<Context>>,
        tx: Sender<AgentMessage>,
        tool_call_tx: Sender<ToolCallRequest>,
    ) -> () {
        let c = context.lock().await;
        let response_stream = llm.stream_response(&c);
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
                        tx.send(AgentMessage::Text {
                            message_id: id.clone(),
                            chunk,
                            is_complete: false,
                            model_name: model_name.clone(),
                        });
                    }
                }

                Ok(ToolRequestStart { id, name }) => {
                    tx.send(AgentMessage::ToolCall {
                        tool_call_id: id,
                        name,
                        arguments: None,
                        result: None,
                        is_complete: false,
                        model_name: model_name.clone(),
                    });
                }

                Ok(ToolRequestArg { id, chunk }) => {
                    tx.send(AgentMessage::ToolCall {
                        tool_call_id: id,
                        name: String::new(),
                        arguments: Some(chunk.to_string()),
                        result: None,
                        is_complete: false,
                        model_name: model_name.clone(),
                    });
                }

                Ok(ToolRequestComplete { tool_call }) => {
                    tracing::debug!(
                        "Tool request completed: {} ({})",
                        tool_call.name,
                        tool_call.id
                    );

                    tool_call_requests.push(tool_call.clone());
                    tool_call_tx.send(tool_call.clone());

                    tx.send(AgentMessage::ToolCall {
                        tool_call_id: tool_call.id,
                        name: String::new(),
                        arguments: Some(tool_call.arguments),
                        result: None,
                        is_complete: false,
                        model_name: model_name.clone(),
                    });
                }

                Ok(Done) => {
                    break;
                }

                Ok(Error { message }) => {
                    tx.send(AgentMessage::Error {
                        message: message.to_string(),
                    });
                    return;
                }

                Err(e) => {
                    tx.send(AgentMessage::Error {
                        message: e.to_string(),
                    });
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

            tx.send(AgentMessage::Text {
                message_id: id.clone(),
                chunk: message_content,
                is_complete: true,
                model_name: model_name.clone(),
            });
        }
    }
}

struct ToolExecutor {}

impl ToolExecutor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn start(
        mcp: Arc<Mutex<McpManager>>,
    ) -> (
        JoinHandle<()>,
        Sender<ToolCallRequest>,
        Receiver<ToolCallResult>,
    ) {
        let (tool_call_tx, mut tool_call_rx) = mpsc::channel::<ToolCallRequest>(100);
        let (tool_result_tx, tool_result_rx) = mpsc::channel::<ToolCallResult>(100);

        let handle = tokio::spawn(async move {
            while let Some(request) = tool_call_rx.recv().await {
                let result_str = match serde_json::from_str(&request.arguments) {
                    Ok(args) => {
                        let mcp_client_guard = mcp.lock().await;
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

                tool_result_tx
                    .send(ToolCallResult {
                        id: request.id.clone(),
                        name: request.name.clone(),
                        arguments: request.arguments,
                        result: result_str,
                    })
                    .await;
            }
        });

        (handle, tool_call_tx, tool_result_rx)
    }
}

struct ToolCallManager {
    requests: Vec<ToolCallRequest>,
    tool_result_rx: mpsc::Receiver<ToolCallResult>,
    tool_result_tx: mpsc::Sender<ToolCallResult>,
    completed_results: Vec<ToolCallResult>,
    tool_handles: Vec<JoinHandle<std::result::Result<(), SendError<ToolCallResult>>>>,
}

impl ToolCallManager {
    pub fn new() -> Self {
        let (tool_result_tx, tool_result_rx) = mpsc::channel(100);
        Self {
            requests: Vec::new(),
            tool_result_rx,
            tool_result_tx,
            completed_results: Vec::new(),
            tool_handles: Vec::new(),
        }
    }

    pub fn execute_tool(&mut self, mcp_client: Arc<Mutex<McpManager>>, request: ToolCallRequest) {
        self.requests.push(request.clone());
        let tx = self.tool_result_tx.clone();
        let handle = tokio::spawn(async move {
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

            tx.send(ToolCallResult {
                id: request.id.clone(),
                name: request.name.clone(),
                arguments: request.arguments,
                result: result_str,
            })
            .await
        });
        self.tool_handles.push(handle);
    }

    pub async fn wait_for_all_tools_to_execute(&mut self) -> Vec<ToolCallResult> {
        let handles = std::mem::take(&mut self.tool_handles);
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
