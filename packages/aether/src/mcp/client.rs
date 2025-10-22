// Don't use custom Result type here as we need to return rmcp::ErrorData
use rmcp::{
    ClientHandler, RoleClient,
    model::{
        ClientInfo, CreateElicitationRequestParam, CreateElicitationResult, ElicitationAction,
        ErrorData, ProgressNotificationParam,
    },
    service::{NotificationContext, RequestContext},
};
use std::result::Result;
use tokio::sync::{mpsc, oneshot};

use crate::llm::{ToolCallProgress, ToolCallStatus};
use crate::mcp::{manager::ProgressChannelMap, ElicitationRequest};

/// MCP client handler for Aether
///
/// This client handles incoming requests and notifications from MCP servers, including:
/// - Elicitation requests (prompting the user for input)
/// - Progress notifications from long-running tool executions
pub struct McpClient {
    client_info: ClientInfo,
    elicitation_sender: mpsc::Sender<ElicitationRequest>,
    progress_channels: ProgressChannelMap,
}

impl McpClient {
    pub fn new(
        client_info: ClientInfo,
        elicitation_sender: mpsc::Sender<ElicitationRequest>,
        progress_channels: ProgressChannelMap,
    ) -> Self {
        Self {
            client_info,
            elicitation_sender,
            progress_channels,
        }
    }
}

impl ClientHandler for McpClient {
    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }

    async fn create_elicitation(
        &self,
        request: CreateElicitationRequestParam,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, ErrorData> {
        let (response_tx, response_rx) = oneshot::channel();
        let elicitation_request = ElicitationRequest {
            request,
            response_sender: response_tx,
        };

        match self.elicitation_sender.send(elicitation_request).await {
            Ok(_) => match response_rx.await {
                Ok(result) => Ok(result),
                Err(_) => Ok(CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                }),
            },

            Err(_) => Ok(CreateElicitationResult {
                action: ElicitationAction::Decline,
                content: None,
            }),
        }
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        // Look up the channel for this progress token
        let progress_token = params.progress_token.to_string();
        tracing::debug!("Received progress notification for token: {}", progress_token);

        let channel = {
            let channels = self.progress_channels.lock().await;
            channels.get(&progress_token).cloned()
        };

        if let Some(tx) = channel {
            let status = ToolCallStatus::InProgress {
                id: progress_token.clone(),
                progress: ToolCallProgress {
                    progress: params.progress,
                    total: params.total,
                    message: params.message.map(|s| s.to_string()),
                },
            };

            if let Err(e) = tx.send(status).await {
                tracing::warn!(
                    "Failed to send progress update for token {}: {}",
                    progress_token,
                    e
                );
            }
        } else {
            tracing::debug!(
                "No channel registered for progress token: {}",
                progress_token
            );
        }
    }
}
