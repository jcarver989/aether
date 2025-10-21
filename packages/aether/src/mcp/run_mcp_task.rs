use crate::llm::{ToolCallError, ToolCallRequest, ToolCallResult, ToolDefinition};
use crate::mcp::McpManager;
use rmcp::model::{GetPromptResult, Prompt};
use rmcp::{RoleClient, model::CallToolRequestParam};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Commands that can be sent to the MCP manager task
#[derive(Debug)]
pub enum McpCommand {
    ExecuteTool {
        request: ToolCallRequest,
        tx: oneshot::Sender<Result<ToolCallResult, ToolCallError>>,
    },
    ListPrompts {
        tx: oneshot::Sender<Result<Vec<Prompt>, String>>,
    },
    GetPrompt {
        name: String,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
        tx: oneshot::Sender<Result<GetPromptResult, String>>,
    },
}

/// Events emitted by the MCP manager task
#[derive(Debug, Clone)]
pub enum McpEvent {
    ToolsChanged(Vec<ToolDefinition>),
}

pub async fn run_mcp_task(mut mcp: McpManager, mut command_rx: mpsc::Receiver<McpCommand>) {
    while let Some(command) = command_rx.recv().await {
        match command {
            McpCommand::ExecuteTool { request, tx } => match mcp.get_client_for_tool(&request.name)
            {
                Ok(client) => {
                    tokio::spawn(async move {
                        let result = try_execute_tool(client, &request).await;
                        let _ = tx.send(result);
                    });
                }

                Err(e) => {
                    tracing::error!("Failed to get client for tool {}: {e}", request.name);
                    let error = ToolCallError {
                        id: request.id.clone(),
                        name: request.name.clone(),
                        arguments: Some(request.arguments.clone()),
                        error: format!("Failed to get client: {e}"),
                    };
                    let _ = tx.send(Err(error));
                }
            },

            McpCommand::ListPrompts { tx } => {
                let result = mcp
                    .list_prompts()
                    .await
                    .map_err(|e| format!("Failed to list prompts: {e}"));
                let _ = tx.send(result);
            }

            McpCommand::GetPrompt {
                name: namespaced_name,
                arguments,
                tx,
            } => {
                let result = mcp
                    .get_prompt(&namespaced_name, arguments)
                    .await
                    .map_err(|e| format!("Failed to get prompt: {e}"));
                let _ = tx.send(result);
            }
        }
    }

    mcp.shutdown().await;
    tracing::debug!("MCP manager task ended");
}

async fn try_execute_tool(
    client: rmcp::Peer<RoleClient>,
    request: &ToolCallRequest,
) -> Result<ToolCallResult, ToolCallError> {
    let tool_request = CallToolRequestParam::try_from(request).map_err(|e| ToolCallError {
        id: request.id.clone(),
        name: request.name.clone(),
        arguments: Some(request.arguments.clone()),
        error: e,
    })?;

    let mcp_result = timeout(TOOL_EXECUTION_TIMEOUT, client.call_tool(tool_request))
        .await
        .map_err(|_| ToolCallError {
            id: request.id.clone(),
            name: request.name.clone(),
            arguments: Some(request.arguments.clone()),
            error: format!("Tool execution timed out after {TOOL_EXECUTION_TIMEOUT:?}"),
        })?
        .map_err(|e| ToolCallError {
            id: request.id.clone(),
            name: request.name.clone(),
            arguments: Some(request.arguments.clone()),
            error: format!("Tool execution failed: {e}"),
        })?;

    ToolCallResult::try_from((request, mcp_result))
}
