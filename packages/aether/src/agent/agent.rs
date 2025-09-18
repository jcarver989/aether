use crate::agent::AgentMessage;
use crate::agent::Result;
use crate::agent::UserMessage;
use crate::agent::process_llm_stream_task::ProcessLlmStreamTask;
use crate::agent::tool_executor_task::ToolExecutorTask;
use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::mcp::McpManager;
use crate::mcp::manager::parse_namespaced_tool_name;
use crate::types::ChatMessage;
use crate::types::IsoString;
use crate::types::ToolDefinition;
use futures::Stream;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

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
        let (tx, rx) = mpsc::channel::<AgentMessage>(100);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        let llm = self.llm.clone();
        let context = self.context.clone();
        let mcp = self.mcp.clone();

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
                let (llm_handle, _tool_executor_handle, mut tool_result_rx, mut llm_rx) = {
                    let (llm_tx, llm_rx) = mpsc::channel::<AgentMessage>(100);

                    let (tool_executor_handle, tool_call_tx, tool_result_rx) =
                        ToolExecutorTask::new().start(mcp.clone());

                    let handle = ProcessLlmStreamTask::run(
                        llm.clone(),
                        context.clone(),
                        llm_tx,
                        tool_call_tx.clone(),
                    );

                    (handle, tool_executor_handle, tool_result_rx, llm_rx)
                };

                let mut llm_finished = false;
                let mut tools_finished = false;
                let mut has_tool_calls = false;

                loop {
                    tokio::select! {
                        llm_msg = llm_rx.recv() => {
                            tracing::trace!("Received LLM message: {:?}", llm_msg.is_some());
                            match llm_msg {

                                Some(msg) => {
                                    tracing::trace!("Forwarding LLM message to output");
                                    if let Err(e) = tx.send(msg).await {
                                        tracing::warn!("Failed to send LLM message to output: {:?}", e);
                                    }
                                }

                                None => {
                                    tracing::debug!("LLM completed, dropped tool_call_tx");
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
                                    tools_finished = true;
                                }
                            }
                        }
                    }

                    if llm_finished && tools_finished {
                        tracing::debug!("Agent: Completed inner loop; both tools and llm finished");
                        break;
                    }
                }

                let _ = llm_handle.await;

                if !has_tool_calls {
                    break;
                }

                _current_iteration += 1;
            }
        });
        return stream;
    }
}

#[derive(Clone)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub result: String,
}
