use crate::agent::AgentMessage;
use crate::agent::ToolCallResult;
use crate::agent::process_llm_stream_task::ProcessLlmStreamTask;
use crate::agent::tool_executor_task::ToolExecutorTask;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::McpManager;
use crate::mcp::manager::parse_namespaced_tool_name;
use crate::types::ChatMessage;
use crate::types::IsoString;
use crate::types::ToolDefinition;
use futures::future::join_all;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{Mutex as TokioMutex, mpsc};
use tokio::task::JoinHandle;
use tracing::Level;
use tracing::{debug, span, trace};

pub struct AgentTask {}

impl AgentTask {
    pub fn run<T: ModelProvider + 'static>(
        llm: Arc<T>,
        mcp: Arc<TokioMutex<McpManager>>,
        context: Arc<Mutex<Context>>,
    ) -> (JoinHandle<()>, Receiver<AgentMessage>) {
        let (tx, rx) = mpsc::channel::<AgentMessage>(100);
        let handle = tokio::spawn(async move {
            let span = span!(Level::DEBUG, "agent_task");
            let _guard = span.enter();
            {
                let mut mcp = mcp.lock().await;
                let update_tools_result = mcp.discover_tools().await;

                if let Err(e) = update_tools_result {
                    let _ = tx
                        .send(AgentMessage::Error {
                            message: format!("Error fetching tools: {:?}", e),
                        })
                        .await;
                    return;
                }

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

                context.lock().unwrap().set_tools(tools);
            };

            // "Agentic" loop.
            // Each iteration of the outer loop procesess 1 LLM call
            // Each iteration of the inner loop processes 1 streaming "event" chunk from the LLM's response
            loop {
                let model_name = llm.display_name();

                let mut llm_finished = false;
                let mut tools_finished = false;
                let mut has_tool_calls = false;

                let mut tool_results = Vec::<ToolCallResult>::new();
                let mut final_message = String::new();

                let (llm_handle, tool_executor_handle, mut tool_result_rx, mut llm_rx) = {
                    let (llm_tx, llm_rx) = mpsc::channel::<AgentMessage>(100);

                    let (tool_executor_handle, tool_call_tx, tool_result_rx) =
                        ToolExecutorTask::new().run(mcp.clone());

                    let handle = ProcessLlmStreamTask::run(
                        llm.clone(),
                        context.clone(),
                        llm_tx,
                        tool_call_tx.clone(),
                    );

                    (handle, tool_executor_handle, tool_result_rx, llm_rx)
                };

                loop {
                    tokio::select! {
                        llm_msg = llm_rx.recv() => {
                            match llm_msg {

                                Some(AgentMessage::Text { chunk, is_complete: true,.. }) => {
                                    final_message = chunk;
                                }

                                Some(msg) => {
                                    if let Err(e) = tx.send(msg).await {
                                        tracing::warn!("Failed to send LLM message to output: {:?}", e);
                                    }
                                }

                                None => {
                                    llm_finished = true;
                                }
                            }
                        }

                        tool_result = tool_result_rx.recv() => {
                            tracing::trace!("Tool result channel event: {:?}", tool_result.is_some());
                            match tool_result {
                                Some(result) => {
                                    has_tool_calls = true;
                                    tracing::debug!(
                                        "Tool result received: {} -> {}",
                                        result.name,
                                        result.result.len()
                                    );
                                    tracing::trace!("Processing tool result for tool_call_id: {}", result.id);
                                    tool_results.push(result.clone());
                                    let msg = AgentMessage::ToolCall {
                                        tool_call_id: result.id.clone(),
                                        name: result.name.clone(),
                                        arguments: Some(result.arguments.clone()),
                                        result: Some(result.result.clone()),
                                        is_complete: true,
                                        model_name: model_name.clone(),
                                    };

                                    if let Err(e) = tx.send(msg).await {
                                        tracing::warn!("Failed to send ToolCall completion message: {:?}", e);
                                    }
                                }
                                None => {
                                    tools_finished = true;
                                }
                            }
                        }
                    }

                    if llm_finished && tools_finished {
                        tracing::debug!("Agent: Completed inner loop; both tools and llm finished");

                        let mut tool_requests = Vec::new();
                        let mut c = context.lock().unwrap();

                        for result in &tool_results {
                            tool_requests.push(result.request.clone());
                        }

                        c.add_message(ChatMessage::Assistant {
                            content: final_message,
                            timestamp: IsoString::now(),
                            tool_calls: tool_requests,
                        });

                        for result in &tool_results {
                            c.add_message(ChatMessage::ToolCallResult {
                                tool_call_id: result.id.clone(),
                                content: result.result.clone(),
                                timestamp: IsoString::now(),
                            });
                        }

                        break;
                    }
                }

                join_all(vec![llm_handle, tool_executor_handle]).await;
                if !has_tool_calls {
                    break;
                }
            }
        });

        (handle, rx)
    }
}
