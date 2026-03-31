use mcp_utils::client::mcp_client::McpClient;
use mcp_utils::client::{McpManager, McpServerStatusEntry};
use mcp_utils::display_meta::ToolResultMeta;

use futures::future::Either;
use futures::stream::{self, StreamExt};
use llm::{ToolCallError, ToolCallRequest, ToolCallResult, ToolDefinition};
use rmcp::RoleClient;
use rmcp::model::{CallToolRequestParams, GetPromptResult, ProgressNotificationParam, Prompt};
use rmcp::service::RunningService;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// Events emitted during tool execution lifecycle
#[derive(Debug)]
pub enum ToolExecutionEvent {
    Started { tool_id: String, tool_name: String },
    Progress { tool_id: String, progress: ProgressNotificationParam },
    Complete { tool_id: String, result: Result<ToolCallResult, ToolCallError>, result_meta: Option<ToolResultMeta> },
}

type AuthResult = Result<(Vec<McpServerStatusEntry>, Vec<ToolDefinition>), String>;

/// Commands that can be sent to the MCP manager task
#[derive(Debug)]
pub enum McpCommand {
    ExecuteTool {
        request: ToolCallRequest,
        timeout: Duration,
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
    GetServerStatuses {
        tx: oneshot::Sender<Vec<McpServerStatusEntry>>,
    },
    AuthenticateServer {
        name: String,
        tx: oneshot::Sender<AuthResult>,
    },
}

pub async fn run_mcp_task(mut mcp: McpManager, mut command_rx: mpsc::Receiver<McpCommand>) {
    while let Some(command) = command_rx.recv().await {
        on_command(command, &mut mcp).await;
    }

    mcp.shutdown().await;
    tracing::debug!("MCP manager task ended");
}

async fn on_command(command: McpCommand, mcp: &mut McpManager) {
    match command {
        McpCommand::ExecuteTool { request, timeout, tx } => {
            let tool_id = request.id.clone();
            let tool_name = request.name.clone();

            let _ =
                tx.send(ToolExecutionEvent::Started { tool_id: tool_id.clone(), tool_name: tool_name.clone() }).await;

            match mcp.get_client_for_tool(&request.name, &request.arguments) {
                Ok((client, params)) => {
                    tokio::spawn(async move {
                        let outcome =
                            execute_mcp_call(client, &request, params, timeout, tool_id.clone(), tx.clone()).await;
                        let (result, result_meta) = match outcome {
                            Ok((r, m)) => (Ok(r), m),
                            Err(e) => (Err(e), None),
                        };
                        let _ = tx.send(ToolExecutionEvent::Complete { tool_id, result, result_meta }).await;
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to get client for tool {}: {e}", request.name);
                    let error = ToolCallError::from_request(&request, format!("Failed to get client: {e}"));
                    let _ =
                        tx.send(ToolExecutionEvent::Complete { tool_id, result: Err(error), result_meta: None }).await;
                }
            }
        }

        McpCommand::ListPrompts { tx } => {
            let result = mcp.list_prompts().await.map_err(|e| format!("Failed to list prompts: {e}"));
            let _ = tx.send(result);
        }

        McpCommand::GetPrompt { name: namespaced_name, arguments, tx } => {
            let result =
                mcp.get_prompt(&namespaced_name, arguments).await.map_err(|e| format!("Failed to get prompt: {e}"));
            let _ = tx.send(result);
        }

        McpCommand::GetServerStatuses { tx } => {
            let _ = tx.send(mcp.server_statuses().to_vec());
        }

        McpCommand::AuthenticateServer { name, tx } => {
            let result = match mcp.authenticate_server(&name).await {
                Ok(()) => Ok((mcp.server_statuses().to_vec(), mcp.tool_definitions())),
                Err(e) => Err(format!("Authentication failed for '{name}': {e}")),
            };
            let _ = tx.send(result);
        }
    }
}

/// Shared logic for sending an MCP tool call, streaming progress events,
/// and collecting the result.
async fn execute_mcp_call(
    client: Arc<RunningService<RoleClient, McpClient>>,
    request: &ToolCallRequest,
    params: CallToolRequestParams,
    timeout: Duration,
    tool_call_id: String,
    event_tx: mpsc::Sender<ToolExecutionEvent>,
) -> Result<(ToolCallResult, Option<ToolResultMeta>), ToolCallError> {
    use super::tool_bridge::mcp_result_to_tool_call_result;
    use rmcp::model::{ClientRequest::CallToolRequest, Request, ServerResult};
    use rmcp::service::PeerRequestOptions;

    let handle = client
        .send_cancellable_request(CallToolRequest(Request::new(params)), {
            let mut opts = PeerRequestOptions::default();
            opts.timeout = Some(timeout);
            opts
        })
        .await
        .map_err(|e| ToolCallError::from_request(request, format!("Failed to send tool request: {e}")))?;

    let progress_subscriber = client.service().progress_dispatcher.subscribe(handle.progress_token.clone()).await;

    let progress_stream = progress_subscriber
        .map(move |progress| Either::Left(ToolExecutionEvent::Progress { tool_id: tool_call_id.clone(), progress }));

    let result_stream = stream::once(handle.await_response()).map(Either::Right);
    let combined_stream = stream::select(progress_stream, result_stream);
    tokio::pin!(combined_stream);

    let server_result = loop {
        match combined_stream.next().await {
            Some(Either::Left(progress_event)) => {
                let _ = event_tx.send(progress_event).await;
            }
            Some(Either::Right(result)) => {
                break result
                    .map_err(|e| ToolCallError::from_request(request, format!("Tool execution failed: {e}")))?;
            }
            None => {
                return Err(ToolCallError::from_request(request, "Stream ended without result"));
            }
        }
    };

    let ServerResult::CallToolResult(mcp_result) = server_result else {
        return Err(ToolCallError::from_request(request, "Unexpected response type from MCP server"));
    };

    mcp_result_to_tool_call_result(request, mcp_result)
}
