use crate::agent::ToolCallResult;
use crate::mcp::McpManager;
use crate::types::{ToolCallRequest, ToolDefinition};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;

const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Commands that can be sent to the MCP manager task
#[derive(Debug)]
pub enum McpCommand {
    /// Execute a tool call
    ExecuteTool(ToolCallRequest),
    /// Get current tool definitions
    GetToolDefinitions {
        response_tx: tokio::sync::oneshot::Sender<Vec<ToolDefinition>>,
    },
    /// Shutdown the MCP manager
    Shutdown,
}

/// Events emitted by the MCP manager task
#[derive(Debug, Clone)]
pub enum McpEvent {
    /// A tool execution completed
    ToolResult(ToolCallResult),
    /// Tool definitions have changed
    ToolsChanged(Vec<ToolDefinition>),
    /// Error occurred
    Error(String),
}

/// Encapsulates MCP management in a task: tool discovery, execution, and lifecycle
pub struct McpManagerTask {
    pub command_tx: mpsc::Sender<McpCommand>,
    pub handle: JoinHandle<()>,
}

impl McpManagerTask {
    /// Create a new MCP manager task
    /// Returns (task handle, event stream)
    pub fn new(mcp_manager: McpManager) -> (Self, ReceiverStream<McpEvent>) {
        let (command_tx, command_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);

        let handle = tokio::spawn(run_mcp_manager_task(mcp_manager, command_rx, event_tx));
        let event_stream = ReceiverStream::new(event_rx);

        let task = Self { command_tx, handle };
        (task, event_stream)
    }

    /// Send a command to the MCP manager task
    pub async fn send_command(
        &self,
        command: McpCommand,
    ) -> Result<(), mpsc::error::SendError<McpCommand>> {
        self.command_tx.send(command).await
    }

    /// Shutdown the MCP manager task
    pub async fn shutdown(self) {
        let _ = self.command_tx.send(McpCommand::Shutdown).await;
        let _ = self.handle.await;
    }
}

/// MCP manager task loop - processes commands and emits events
async fn run_mcp_manager_task(
    mut mcp: McpManager,
    mut command_rx: mpsc::Receiver<McpCommand>,
    event_tx: mpsc::Sender<McpEvent>,
) {
    while let Some(command) = command_rx.recv().await {
        match command {
            McpCommand::ExecuteTool(request) => {
                handle_tool_execution(&mcp, request, &event_tx).await;
            }

            McpCommand::GetToolDefinitions { response_tx } => {
                let definitions = mcp.tool_definitions();
                let _ = response_tx.send(definitions);
            }

            McpCommand::Shutdown => {
                tracing::debug!("MCP manager task shutting down");
                break;
            }
        }
    }

    // Cleanup
    mcp.shutdown().await;
    tracing::debug!("MCP manager task ended");
}

/// Handle tool execution and send result as event
async fn handle_tool_execution(
    mcp: &McpManager,
    request: ToolCallRequest,
    event_tx: &mpsc::Sender<McpEvent>,
) {
    tracing::trace!(
        "MCP manager executing tool: {} ({})",
        request.name,
        request.id
    );

    let result_str = match serde_json::from_str(&request.arguments) {
        Ok(args) => {
            tracing::trace!("Executing tool {} with parsed args", &request.name);

            // Execute with timeout
            match timeout(TOOL_EXECUTION_TIMEOUT, mcp.execute_tool(&request.name, args)).await {
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
                    format!(
                        "Tool execution timed out after {:?}",
                        TOOL_EXECUTION_TIMEOUT
                    )
                }
            }
        }

        Err(e) => {
            tracing::error!("Invalid tool arguments for {}: {}", &request.name, e);
            format!("Invalid tool arguments: {}", e)
        }
    };

    tracing::debug!("MCP manager completed {} ({})", request.name, request.id);

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

    match event_tx.send(McpEvent::ToolResult(result)).await {
        Ok(_) => {
            tracing::trace!("Successfully sent tool result event");
        }
        Err(e) => {
            tracing::error!("Failed to send tool result event: {:?}", e);
        }
    }
}
