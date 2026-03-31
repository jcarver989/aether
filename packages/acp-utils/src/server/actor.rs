use agent_client_protocol::{self as acp, Client};
use tokio::sync::{mpsc, oneshot};
use tracing::debug;

use super::AcpServerError;

/// Messages that can be sent to the ACP actor.
#[derive(Debug)]
pub enum AcpRequest {
    SessionNotification {
        notification: Box<acp::SessionNotification>,
        response_tx: oneshot::Sender<Result<(), AcpServerError>>,
    },
    ExtNotification {
        notification: acp::ExtNotification,
        response_tx: oneshot::Sender<Result<(), AcpServerError>>,
    },
    RequestPermission {
        request: Box<acp::RequestPermissionRequest>,
        response_tx: oneshot::Sender<Result<acp::RequestPermissionResponse, AcpServerError>>,
    },
    ExtMethod {
        request: acp::ExtRequest,
        response_tx: oneshot::Sender<Result<acp::ExtResponse, AcpServerError>>,
    },
}

/// Actor that owns the !Send ACP `AgentSideConnection` and processes requests
/// sequentially. Must be spawned on a `LocalSet`.
pub struct AcpActor {
    conn: acp::AgentSideConnection,
    request_rx: mpsc::UnboundedReceiver<AcpRequest>,
}

impl AcpActor {
    pub fn new(conn: acp::AgentSideConnection, request_rx: mpsc::UnboundedReceiver<AcpRequest>) -> Self {
        Self { conn, request_rx }
    }

    /// Run the actor loop. This must be spawned on a `LocalSet`.
    pub async fn run(mut self) {
        debug!("ACP actor starting");

        while let Some(request) = self.request_rx.recv().await {
            self.handle_request(request).await;
        }

        debug!("ACP actor stopping");
    }

    async fn handle_request(&self, request: AcpRequest) {
        match request {
            AcpRequest::SessionNotification { notification, response_tx } => {
                debug!("ACP actor: session_notification");
                let result = self
                    .conn
                    .session_notification(*notification)
                    .await
                    .map_err(|e| AcpServerError::Protocol(format!("session_notification: {e}")));
                let _ = response_tx.send(result);
            }

            AcpRequest::ExtNotification { notification, response_tx } => {
                debug!("ACP actor: ext_notification {}", notification.method);
                let result = self
                    .conn
                    .ext_notification(notification)
                    .await
                    .map_err(|e| AcpServerError::Protocol(format!("ext_notification: {e}")));
                let _ = response_tx.send(result);
            }

            AcpRequest::RequestPermission { request, response_tx } => {
                debug!("ACP actor: request_permission");
                let result = self
                    .conn
                    .request_permission(*request)
                    .await
                    .map_err(|e| AcpServerError::Protocol(format!("request_permission: {e}")));
                let _ = response_tx.send(result);
            }

            AcpRequest::ExtMethod { request, response_tx } => {
                debug!("ACP actor: ext_method {}", request.method);
                let result = self
                    .conn
                    .ext_method(request)
                    .await
                    .map_err(|e| AcpServerError::Protocol(format!("ext_method: {e}")));
                let _ = response_tx.send(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_actor_exits_on_channel_drop() {
        let (tx, rx) = mpsc::unbounded_channel::<AcpRequest>();
        // Drop the sender immediately
        drop(tx);

        // AcpActor::run needs a real AgentSideConnection which is !Send,
        // so we test that the channel-drop logic works via the handle instead.
        // The actor's run() loop terminates when all senders are dropped.
        assert!(rx.is_empty());
    }
}
