use color_eyre::Result;
use rmcp::{
    ClientHandler, RoleClient,
    model::{
        ClientInfo, CreateElicitationRequestParam, CreateElicitationResult, ElicitationAction,
        ErrorData,
    },
    service::RequestContext,
};
use std::{f32::consts::E, future::Future};
use tokio::sync::{mpsc, oneshot};

use crate::mcp::ElicitationRequest;

pub struct McpClient {
    client_info: ClientInfo,
    elicitation_sender: mpsc::UnboundedSender<ElicitationRequest>,
}

impl McpClient {
    pub fn new(
        client_info: ClientInfo,
        elicitation_sender: mpsc::UnboundedSender<ElicitationRequest>,
    ) -> Self {
        Self {
            client_info,
            elicitation_sender,
        }
    }
}

impl ClientHandler for McpClient {
    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }

    fn create_elicitation(
        &self,
        request: CreateElicitationRequestParam,
        _context: RequestContext<RoleClient>,
    ) -> impl Future<Output = Result<CreateElicitationResult, ErrorData>> + Send + '_ {
        async move {
            let (response_tx, response_rx) = oneshot::channel();
            let elicitation_request = ElicitationRequest {
                request,
                response_sender: response_tx,
            };

            match self.elicitation_sender.send(elicitation_request) {
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
}
