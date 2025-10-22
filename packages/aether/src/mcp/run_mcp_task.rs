use crate::llm::{ToolCallError, ToolCallRequest, ToolCallResult, ToolCallStatus, ToolDefinition};
use crate::mcp::{manager::ProgressNotification, McpManager};
use rmcp::model::{GetPromptResult, Prompt};
use rmcp::{RoleClient, model::CallToolRequestParam};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Commands that can be sent to the MCP manager task
#[derive(Debug)]
pub enum McpCommand {
    ExecuteTool {
        request: ToolCallRequest,
        tx: mpsc::Sender<ToolCallStatus>,
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

pub async fn run_mcp_task(
    mut mcp: McpManager,
    mut command_rx: mpsc::Receiver<McpCommand>,
    mut progress_rx: mpsc::Receiver<ProgressNotification>,
) {
    // Local registry of progress tokens to their tool execution channels
    let mut progress_channels: HashMap<String, mpsc::Sender<ToolCallStatus>> = HashMap::new();

    loop {
        tokio::select! {
            // Handle incoming commands
            Some(command) = command_rx.recv() => {
                match command {
                    McpCommand::ExecuteTool { request, tx } => match mcp.get_client_for_tool(&request.name)
                    {
                        Ok(client) => {
                            let progress_token = request.id.clone();

                            // Register the progress channel locally
                            progress_channels.insert(progress_token.clone(), tx.clone());

                            tokio::spawn(async move {
                                // Send Started status
                                let _ = tx
                                    .send(ToolCallStatus::Started {
                                        id: request.id.clone(),
                                        name: request.name.clone(),
                                    })
                                    .await;

                                // Execute tool with progress token
                                let result = try_execute_tool(client, &request, &progress_token).await;

                                // Send Complete or Error status
                                let status = match result {
                                    Ok(result) => ToolCallStatus::Complete { result },
                                    Err(error) => ToolCallStatus::Error { error },
                                };
                                let _ = tx.send(status).await;
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
                            let _ = tx.send(ToolCallStatus::Error { error }).await;
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

            // Handle incoming progress notifications from MCP clients
            Some(notification) = progress_rx.recv() => {
                tracing::debug!(
                    "Routing progress notification for token: {}",
                    notification.progress_token
                );

                if let Some(tx) = progress_channels.get(&notification.progress_token) {
                    let status = ToolCallStatus::InProgress {
                        id: notification.progress_token.clone(),
                        progress: notification.progress,
                    };

                    if let Err(e) = tx.send(status).await {
                        tracing::warn!(
                            "Failed to send progress for token {}: {}",
                            notification.progress_token,
                            e
                        );
                        // Channel closed, clean up
                        progress_channels.remove(&notification.progress_token);
                    }
                } else {
                    tracing::debug!(
                        "No active tool for progress token: {}",
                        notification.progress_token
                    );
                }
            }

            // Both channels closed, exit
            else => {
                tracing::debug!("MCP manager channels closed, shutting down");
                break;
            }
        }
    }

    mcp.shutdown().await;
    tracing::debug!("MCP manager task ended");
}

async fn try_execute_tool(
    client: rmcp::Peer<RoleClient>,
    request: &ToolCallRequest,
    progress_token: &str,
) -> Result<ToolCallResult, ToolCallError> {
    let tool_request = CallToolRequestParam::try_from(request).map_err(|e| ToolCallError {
        id: request.id.clone(),
        name: request.name.clone(),
        arguments: Some(request.arguments.clone()),
        error: e,
    })?;

    // TODO: Inject progress token into the MCP request metadata
    // According to MCP spec, this should be added to the request's _meta field:
    // {
    //   "jsonrpc": "2.0",
    //   "id": 1,
    //   "method": "tools/call",
    //   "params": {
    //     "_meta": {
    //       "progressToken": progress_token
    //     },
    //     ...tool_request
    //   }
    // }
    //
    // This requires either:
    // 1. A way to pass metadata to client.call_tool()
    // 2. Using a lower-level rmcp API that exposes request building
    // 3. Extending rmcp Peer to support progress tokens
    tracing::trace!("Executing tool with progress token: {}", progress_token);

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
