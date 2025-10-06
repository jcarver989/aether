use crate::agent::ToolCallResult;
use crate::mcp::McpManager;
use crate::types::{ToolCallRequest, ToolDefinition};
use rmcp::{RoleClient, model::CallToolRequestParam};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Commands that can be sent to the MCP manager task
#[derive(Debug)]
pub enum McpCommand {
    ExecuteTool(ToolCallRequest),
}

/// Events emitted by the MCP manager task
#[derive(Debug, Clone)]
pub enum McpEvent {
    ToolResult(ToolCallResult),
    ToolsChanged(Vec<ToolDefinition>),
}

pub async fn run_mcp_task(
    mut mcp: McpManager,
    mut command_rx: mpsc::Receiver<McpCommand>,
    event_tx: mpsc::Sender<McpEvent>,
) {
    while let Some(command) = command_rx.recv().await {
        match command {
            McpCommand::ExecuteTool(request) => match mcp.get_client_for_tool(&request.name) {
                Ok(client) => {
                    let event_tx = event_tx.clone();
                    tokio::spawn(async move {
                        let result_str = match try_execute_tool(client, &request).await {
                            Ok(result) => result,
                            Err(e) => e,
                        };

                        let result = ToolCallResult {
                            id: request.id.clone(),
                            name: request.name.clone(),
                            arguments: request.arguments.clone(),
                            result: result_str,
                            request,
                        };

                        let _ = event_tx.send(McpEvent::ToolResult(result)).await;
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to get client for tool {}: {}", request.name, e);
                    let error_result = ToolCallResult {
                        id: request.id.clone(),
                        name: request.name.clone(),
                        arguments: request.arguments.clone(),
                        result: format!("Failed to get client: {}", e),
                        request,
                    };
                    let _ = event_tx.send(McpEvent::ToolResult(error_result)).await;
                }
            },
        }
    }

    mcp.shutdown().await;
    tracing::debug!("MCP manager task ended");
}

async fn try_execute_tool(
    client: rmcp::Peer<RoleClient>,
    request: &ToolCallRequest,
) -> Result<String, String> {
    let tool_request = CallToolRequestParam::try_from(request).map_err(|e| e.to_string())?;

    let result = timeout(TOOL_EXECUTION_TIMEOUT, client.call_tool(tool_request))
        .await
        .map_err(|_| {
            format!(
                "Tool execution timed out after {:?}",
                TOOL_EXECUTION_TIMEOUT
            )
        })?
        .map_err(|e| format!("Tool execution failed: {}", e))?;

    Ok(format_tool_result(result))
}

/// Format the tool result from MCP response
fn format_tool_result(result: rmcp::model::CallToolResult) -> String {
    if result.is_error.unwrap_or(false) {
        let error_msg = result
            .content
            .first()
            .map(|content| format!("{content:?}"))
            .unwrap_or_else(|| "Unknown error".to_string());
        format!("Tool execution error: {}", error_msg)
    } else {
        let result_value = result
            .content
            .first()
            .map(|content| {
                serde_json::to_value(content)
                    .unwrap_or(serde_json::Value::String("Serialization error".to_string()))
            })
            .unwrap_or_else(|| serde_json::Value::String("No result".to_string()));
        result_value.to_string()
    }
}
