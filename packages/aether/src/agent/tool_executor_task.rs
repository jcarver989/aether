use crate::agent::ToolCallResult;
use crate::mcp::McpManager;
use crate::types::ToolCallRequest;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Encapsulates tool execution machinery: spawning, channels, and cleanup
pub struct ToolExecutor {
    call_tx: mpsc::Sender<ToolCallRequest>,
    result_rx: mpsc::Receiver<ToolCallResult>,
    handle: JoinHandle<()>,
}

impl ToolExecutor {
    /// Create a new tool executor with the given MCP manager
    pub fn new(mcp: McpManager) -> Self {
        let (call_tx, call_rx) = mpsc::channel(100);
        let (result_tx, result_rx) = mpsc::channel(100);

        let handle = tokio::spawn(run_tool_executor(mcp, call_rx, result_tx));

        Self {
            call_tx,
            result_rx,
            handle,
        }
    }

    /// Send a tool request for execution
    pub async fn send_request(&self, request: ToolCallRequest) -> Result<(), mpsc::error::SendError<ToolCallRequest>> {
        self.call_tx.send(request).await
    }

    /// Receive the next tool result
    pub async fn recv_result(&mut self) -> Option<ToolCallResult> {
        self.result_rx.recv().await
    }

    /// Shutdown the tool executor, closing channels and awaiting completion
    pub async fn shutdown(self) {
        drop(self.call_tx); // Close channel to signal shutdown
        let _ = self.handle.await;
    }
}

/// Tool executor loop - processes tool calls and sends results
pub async fn run_tool_executor(
    mcp: McpManager,
    mut tool_call_rx: mpsc::Receiver<ToolCallRequest>,
    tool_result_tx: mpsc::Sender<ToolCallResult>,
) {
    while let Some(request) = tool_call_rx.recv().await {
        tracing::trace!(
            "Tool executor received request: {} ({})",
            request.name,
            request.id
        );

        let result_str = match serde_json::from_str(&request.arguments) {
            Ok(args) => {
                tracing::trace!("Executing tool {} with parsed args", &request.name);

                // Execute with timeout
                match timeout(TOOL_EXECUTION_TIMEOUT, mcp.execute_tool(&request.name, args)).await
                {
                    Ok(Ok(result)) => {
                        tracing::trace!(
                            "Tool {} execution successful, result length: {}",
                            &request.name,
                            result.to_string().len()
                        );
                        result.to_string()
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Tool {} execution failed: {}", &request.name, e);
                        format!("Tool execution failed: {}", e)
                    }
                    Err(_) => {
                        tracing::error!(
                            "Tool {} execution timed out after {:?}",
                            &request.name,
                            TOOL_EXECUTION_TIMEOUT
                        );
                        format!("Tool execution timed out after {:?}", TOOL_EXECUTION_TIMEOUT)
                    }
                }
            }

            Err(e) => {
                tracing::error!("Invalid tool arguments for {}: {}", &request.name, e);
                format!("Invalid tool arguments: {}", e)
            }
        };

        tracing::debug!("ToolExecutor completed {} ({})", request.name, request.id);

        let result = ToolCallResult {
            id: request.id.clone(),
            name: request.name.clone(),
            arguments: request.arguments.clone(),
            result: result_str,
            request,
        };

        tracing::trace!(
            "Sending tool result for {} ({}) - result length: {}",
            result.name,
            result.id,
            result.result.len()
        );

        match tool_result_tx.send(result).await {
            Ok(_) => {
                tracing::trace!("Successfully sent tool result");
            }
            Err(e) => {
                tracing::error!("Failed to send tool result: {:?}", e);
                break;
            }
        }
    }
    tracing::trace!("Tool executor task ending - tool_call_rx channel closed");
}
