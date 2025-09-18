use crate::agent::ToolCallResult;
use crate::mcp::McpManager;
use crate::types::ToolCallRequest;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tracing::Level;
use tracing::span;

pub struct ToolExecutorTask {}

impl ToolExecutorTask {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run(
        &self,
        mcp: Arc<Mutex<McpManager>>,
    ) -> (
        JoinHandle<()>,
        Sender<ToolCallRequest>,
        Receiver<ToolCallResult>,
    ) {
        let (tool_call_tx, mut tool_call_rx) = mpsc::channel::<ToolCallRequest>(100);
        let (tool_result_tx, tool_result_rx) = mpsc::channel::<ToolCallResult>(100);

        let handle = tokio::spawn(async move {
            let span = span!(Level::DEBUG, "tool_executor_task");
            let _guard = span.enter();

            while let Some(request) = tool_call_rx.recv().await {
                let result_str = match serde_json::from_str(&request.arguments) {
                    Ok(args) => {
                        tracing::trace!("Executing tool {} with parsed args", &request.name);
                        let mcp_client_guard = mcp.lock().await;
                        match mcp_client_guard.execute_tool(&request.name, args).await {
                            Ok(result) => {
                                tracing::trace!(
                                    "Tool {} execution successful, result length: {}",
                                    &request.name,
                                    result.to_string().len()
                                );
                                result.to_string()
                            }
                            Err(e) => {
                                tracing::warn!("Tool {} execution failed: {}", &request.name, e);
                                format!("Tool execution failed: {}", e)
                            }
                        }
                    }

                    Err(e) => {
                        tracing::error!("Invalid tool arguments for {}: {}", &request.name, e);
                        format!("Invalid tool arguments: {}", e)
                    }
                };

                let tool_result = ToolCallResult {
                    id: (&request).id.clone(),
                    name: (&request).name.clone(),
                    arguments: (&request).arguments.clone(),
                    result: result_str.clone(),
                    request: request.clone(),
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

                tracing::debug!("ToolExecutor completed {} ({})", request.name, request.id,);
            }
            tracing::trace!("ToolExecutor task ending - tool_call_rx channel closed");
        });

        (handle, tool_call_tx, tool_result_rx)
    }
}
