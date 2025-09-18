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
use tokio::sync::mpsc::error::SendError;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

pub struct Agent<T: ModelProvider> {
    llm: T,
    mcp: Arc<Mutex<McpManager>>,
    context: Context,
}

impl<T: ModelProvider + 'static> Agent<T> {
    pub fn new(llm: T, mcp_client: McpManager, messages: Vec<ChatMessage>) -> Self {
        Self {
            llm: llm,
            mcp: Arc::new(Mutex::new(mcp_client)),
            context: Context::new(
                messages,
                Vec::new(), // populated when tools are discovered
            ),
        }
    }

    pub async fn current_model_display_name(&self) -> String {
        self.llm.display_name()
    }

    pub async fn send(&mut self, message: UserMessage) -> impl Stream<Item = AgentMessage> {
        match message {
            UserMessage::Text { content } => {
                self.context.add_message(ChatMessage::User {
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
                let model_name = self.llm.display_name();

                let mut tool_call_manager = ToolCallManager::new();
                let llm_stream =  self.process_llm_stream().await;
                pin_mut!(llm_stream);

                while let Some(event) = llm_stream.next().await {
                    match &event {
                        AgentMessage::Text { chunk, is_complete: true, .. } => {
                            self.context.add_message(ChatMessage::Assistant {
                                content: chunk.clone(),
                                timestamp: IsoString::now(),
                                tool_calls: tool_call_manager.requests.clone()
                            });
                        },

                        AgentMessage::ToolCall { tool_call_id, name, arguments: Some(args), is_complete: true, .. } => {
                            tool_call_manager.execute_tool(self.mcp.clone(),
                            ToolCallRequest { id: tool_call_id.clone(), name: name.clone(), arguments: args.clone() });
                        },

                        _ => {}
                    }


                    yield event;
                }


                if tool_call_manager.tool_handles.is_empty() {
                    tracing::debug!("No tool requests, terminating agent loop");
                    return;
                }

                if !tool_call_manager.tool_handles.is_empty() {
                    tracing::debug!("Waiting for {} tool results after stream completion...", tool_call_manager.tool_handles.len());
                    let tool_results = tool_call_manager.wait_for_all_tools_to_execute().await;

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
                }

                current_iteration += 1;
            }
        }
    }

    async fn x() {}

    async fn process_llm_stream(&self) -> impl Stream<Item = AgentMessage> {
        stream! {
            let response_stream = self.llm.stream_response(&self.context);
            let model_name = self.llm.display_name();
            pin_mut!(response_stream);

            let mut current_message_id: Option<String> = None;
            let mut message_content = String::new();

            while let Some(event) = response_stream.next().await {
                use LlmResponse::*;
                match event {
                    Ok(Start { message_id }) => {
                        current_message_id = Some(message_id);
                    }

                    Ok(Text { chunk }) => {
                        message_content.push_str(&chunk);
                        if let Some(ref id) = current_message_id {
                            yield AgentMessage::Text {
                                message_id: id.clone(),
                                chunk,
                                is_complete: false,
                                model_name: model_name.clone(),
                            };
                        }
                    }

                    Ok(ToolRequestStart { id, name }) => {
                        yield AgentMessage::ToolCall {
                            tool_call_id: id,
                            name,
                            arguments: None,
                            result: None,
                            is_complete: false,
                            model_name: model_name.clone(),
                        };
                    }

                    Ok(ToolRequestArg { id, chunk }) => {
                        yield AgentMessage::ToolCall {
                            tool_call_id: id,
                            name: String::new(),
                            arguments: Some(chunk.to_string()),
                            result: None,
                            is_complete: false,
                            model_name: model_name.clone(),
                        };
                    }

                    Ok(ToolRequestComplete { tool_call }) => {
                        tracing::debug!(
                            "Tool request completed: {} ({})",
                            tool_call.name,
                            tool_call.id
                        );

                        yield AgentMessage::ToolCall {
                            tool_call_id: tool_call.id,
                            name: String::new(),
                            arguments: Some(tool_call.arguments),
                            result: None,
                            is_complete: false,
                            model_name: model_name.clone(),
                        }

                        //tool_call_manager.execute_tool(self.mcp.clone(), tool_call.clone());
                    }

                    Ok(Done) => {
                        break;
                    }

                    Ok(Error { message }) => {
                        yield AgentMessage::Error {
                            message: message.to_string(),
                        };
                        return;
                    }

                    Err(e) => {
                        yield AgentMessage::Error {
                            message: e.to_string(),
                        };
                        return;
                    }
                }
            }

            if let Some(ref id) = current_message_id {
                yield AgentMessage::Text {
                    message_id: id.clone(),
                    chunk: message_content,
                    is_complete: true,
                    model_name: model_name.clone()
                };
            }
        }
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
