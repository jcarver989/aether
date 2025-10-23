// Don't use custom Result type here as we need to return rmcp::ErrorData
use rmcp::{
    ClientHandler, RoleClient,
    handler::client::progress::ProgressDispatcher,
    model::{
        ClientInfo, CreateElicitationRequestParam, CreateElicitationResult, ElicitationAction,
        ErrorData, ProgressNotificationParam,
    },
    service::{NotificationContext, RequestContext},
};
use std::result::Result;
use tokio::sync::{mpsc, oneshot};

use crate::mcp::ElicitationRequest;

pub struct McpClient {
    client_info: ClientInfo,
    pub progress_dispatcher: ProgressDispatcher,
    elicitation_sender: mpsc::Sender<ElicitationRequest>,
}

impl McpClient {
    pub fn new(
        client_info: ClientInfo,
        elicitation_sender: mpsc::Sender<ElicitationRequest>,
    ) -> Self {
        Self {
            client_info,
            progress_dispatcher: ProgressDispatcher::new(),
            elicitation_sender,
        }
    }
}

impl ClientHandler for McpClient {
    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) -> () {
        self.progress_dispatcher.handle_notification(params).await;
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
}
