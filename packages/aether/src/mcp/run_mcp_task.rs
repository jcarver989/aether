use crate::llm::{ToolCallError, ToolCallRequest, ToolCallResult, ToolDefinition};
use crate::mcp::McpManager;
use futures::future::Either;
use futures::stream::{self, StreamExt};
use rmcp::model::{GetPromptResult, ProgressNotificationParam, Prompt};
use rmcp::service::RunningService;
use rmcp::{RoleClient, model::CallToolRequestParam};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use super::client::McpClient;

const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Events emitted during tool execution lifecycle
#[derive(Debug)]
pub enum ToolExecutionEvent {
    Started {
        tool_id: String,
        tool_name: String,
    },
    Progress {
        tool_id: String,
        progress: ProgressNotificationParam,
    },
    Complete {
        tool_id: String,
        result: Result<ToolCallResult, ToolCallError>,
    },
}

/// Commands that can be sent to the MCP manager task
#[derive(Debug)]
pub enum McpCommand {
    ExecuteTool {
        request: ToolCallRequest,
        tx: mpsc::Sender<ToolExecutionEvent>,
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
        on_command(command, &mcp).await;
    }

    mcp.shutdown().await;
    tracing::debug!("MCP manager task ended");
}

async fn on_command(command: McpCommand, mcp: &McpManager) {
    match command {
        McpCommand::ExecuteTool { request, tx } => {
            let tool_id = request.id.clone();
            let tool_name = request.name.clone();

            let _ = tx
                .send(ToolExecutionEvent::Started {
                    tool_id: tool_id.clone(),
                    tool_name: tool_name.clone(),
                })
                .await;

            match mcp.get_client_for_tool(&request.name) {
                Ok(client) => {
                    tokio::spawn(async move {
                        let result =
                            try_execute_tool(client, &request, tool_id.clone(), tx.clone()).await;
                        let _ = tx
                            .send(ToolExecutionEvent::Complete { tool_id, result })
                            .await;
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
                    let _ = tx
                        .send(ToolExecutionEvent::Complete {
                            tool_id,
                            result: Err(error),
                        })
                        .await;
                }
            }
        }

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

async fn try_execute_tool(
    client: Arc<RunningService<RoleClient, McpClient>>,
    request: &ToolCallRequest,
    tool_call_id: String,
    event_tx: mpsc::Sender<ToolExecutionEvent>,
) -> Result<ToolCallResult, ToolCallError> {
    use rmcp::model::{ClientRequest::CallToolRequest, Request, ServerResult};
    use rmcp::service::PeerRequestOptions;

    let tool_request_param =
        CallToolRequestParam::try_from(request).map_err(|e| ToolCallError {
            id: request.id.clone(),
            name: request.name.clone(),
            arguments: Some(request.arguments.clone()),
            error: e,
        })?;

    let handle = client
        .send_cancellable_request(
            CallToolRequest(Request::new(tool_request_param)),
            PeerRequestOptions {
                timeout: Some(TOOL_EXECUTION_TIMEOUT),
                meta: None,
            },
        )
        .await
        .map_err(|e| ToolCallError {
            id: request.id.clone(),
            name: request.name.clone(),
            arguments: Some(request.arguments.clone()),
            error: format!("Failed to send tool request: {e}"),
        })?;

    let progress_subscriber = client
        .service()
        .progress_dispatcher
        .subscribe(handle.progress_token.clone())
        .await;

    let progress_stream = progress_subscriber.map(move |progress| {
        Either::Left(ToolExecutionEvent::Progress {
            tool_id: tool_call_id.clone(),
            progress,
        })
    });

    let result_stream = stream::once(handle.await_response()).map(Either::Right);
    let combined_stream = stream::select(progress_stream, result_stream);
    tokio::pin!(combined_stream);

    let server_result = loop {
        match combined_stream.next().await {
            Some(Either::Left(progress_event)) => {
                let _ = event_tx.send(progress_event).await;
            }
            Some(Either::Right(result)) => {
                break result.map_err(|e| ToolCallError {
                    id: request.id.clone(),
                    name: request.name.clone(),
                    arguments: Some(request.arguments.clone()),
                    error: format!("Tool execution failed: {e}"),
                })?;
            }
            None => {
                return Err(ToolCallError {
                    id: request.id.clone(),
                    name: request.name.clone(),
                    arguments: Some(request.arguments.clone()),
                    error: "Stream ended without result".to_string(),
                });
            }
        }
    };

    let mcp_result = match server_result {
        ServerResult::CallToolResult(result) => result,
        _ => {
            return Err(ToolCallError {
                id: request.id.clone(),
                name: request.name.clone(),
                arguments: Some(request.arguments.clone()),
                error: "Unexpected response type from MCP server".to_string(),
            });
        }
    };

    ToolCallResult::try_from((request, mcp_result))
}
