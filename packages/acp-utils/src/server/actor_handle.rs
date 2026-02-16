use agent_client_protocol as acp;
use tokio::sync::{mpsc, oneshot};

use super::{AcpRequest, AcpServerError};

/// Send-safe handle to communicate with an [`AcpActor`](super::AcpActor).
#[derive(Clone, Debug)]
pub struct AcpActorHandle {
    request_tx: mpsc::UnboundedSender<AcpRequest>,
}

impl AcpActorHandle {
    pub fn new(request_tx: mpsc::UnboundedSender<AcpRequest>) -> Self {
        Self { request_tx }
    }

    pub async fn send_session_notification(
        &self,
        notification: acp::SessionNotification,
    ) -> Result<(), AcpServerError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::SessionNotification {
                notification: Box::new(notification),
                response_tx,
            })
            .map_err(|_| AcpServerError::ActorStopped)?;
        response_rx
            .await
            .map_err(|_| AcpServerError::ActorStopped)?
    }

    pub async fn send_ext_notification(
        &self,
        notification: acp::ExtNotification,
    ) -> Result<(), AcpServerError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(AcpRequest::ExtNotification {
                notification,
                response_tx,
            })
            .map_err(|_| AcpServerError::ActorStopped)?;
        response_rx
            .await
            .map_err(|_| AcpServerError::ActorStopped)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_returns_error_when_actor_stopped() {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = AcpActorHandle::new(tx);

        // Drop the receiver to simulate a stopped actor
        drop(rx);

        let session_id = acp::SessionId::new("test");
        let notification = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("test"),
            ))),
        );

        let result = handle.send_session_notification(notification).await;
        assert!(matches!(result, Err(AcpServerError::ActorStopped)));
    }

    #[tokio::test]
    async fn test_ext_handle_returns_error_when_actor_stopped() {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = AcpActorHandle::new(tx);
        drop(rx);

        let null_value: std::sync::Arc<serde_json::value::RawValue> =
            serde_json::from_str("null").expect("null is valid JSON");
        let notification = acp::ExtNotification::new("test/method", null_value);
        let result = handle.send_ext_notification(notification).await;
        assert!(matches!(result, Err(AcpServerError::ActorStopped)));
    }
}
