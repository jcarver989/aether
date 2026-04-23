use agent_client_protocol::schema::{RequestPermissionRequest, RequestPermissionResponse, SessionNotification};
use agent_client_protocol::{Client, ConnectionTo, JsonRpcNotification, JsonRpcRequest};
use std::sync::{Arc, OnceLock};
use tracing::trace;

use super::AcpServerError;

/// Send-safe handle that issues outbound ACP traffic directly on a
/// [`ConnectionTo<Client>`]. Cheap to clone.
///
/// The handle starts [`AcpConnectionHandle::new_disconnected`] so collaborators
/// can wire up the rest of the runtime before the transport is ready. Once the
/// ACP runtime produces the connection, call [`AcpConnectionHandle::attach`]
/// with it exactly once. Sends before attach fail with
/// [`AcpServerError::ConnectionUnavailable`]; sends after the transport has
/// closed surface the underlying protocol error through
/// [`AcpServerError::Protocol`].
///
/// Internally the connection is stored in a [`OnceLock`], which enforces the
/// one-shot invariant by construction — calling `attach` twice panics.
#[derive(Clone, Debug, Default)]
pub struct AcpConnectionHandle {
    connection: Arc<OnceLock<ConnectionTo<Client>>>,
}

impl AcpConnectionHandle {
    pub fn new_disconnected() -> Self {
        Self::default()
    }

    /// Attach the live connection. Must be called exactly once per handle;
    /// subsequent calls panic.
    pub fn attach(&self, conn: ConnectionTo<Client>) {
        self.connection.set(conn).expect("AcpConnectionHandle::attach called more than once");
    }

    pub fn send_session_notification(&self, notification: SessionNotification) -> Result<(), AcpServerError> {
        trace!("ACP handle: session_notification");
        self.current()?.send_notification(notification).map_err(|e| AcpServerError::protocol("session_notification", e))
    }

    /// Send a typed ext notification whose wire method name is baked into the
    /// type's [`JsonRpcNotification`] impl (e.g. [`crate::notifications::ContextUsageParams`]).
    pub fn send_notification<N: JsonRpcNotification>(&self, notification: N) -> Result<(), AcpServerError> {
        let method = notification.method().to_string();
        trace!(%method, "ACP handle: send_notification");
        self.current()?.send_notification(notification).map_err(|e| AcpServerError::protocol_owned(method, e))
    }

    pub async fn request_permission(
        &self,
        request: RequestPermissionRequest,
    ) -> Result<RequestPermissionResponse, AcpServerError> {
        trace!("ACP handle: request_permission");
        let task = self.current()?.send_request(request);
        task.block_task().await.map_err(|e| AcpServerError::protocol("request_permission", e))
    }

    /// Send a typed ext request and await its typed response. Wire method name
    /// comes from the request type's [`JsonRpcRequest`] impl.
    pub async fn send_request<R: JsonRpcRequest>(&self, request: R) -> Result<R::Response, AcpServerError> {
        let method = request.method().to_string();
        trace!(%method, "ACP handle: send_request");
        let task = self.current()?.send_request(request);
        task.block_task().await.map_err(|e| AcpServerError::protocol_owned(method, e))
    }

    fn current(&self) -> Result<ConnectionTo<Client>, AcpServerError> {
        self.connection.get().cloned().ok_or(AcpServerError::ConnectionUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::ContextClearedParams;
    use agent_client_protocol::schema as acp;

    #[test]
    fn unattached_handle_returns_connection_unavailable() {
        let handle = AcpConnectionHandle::new_disconnected();
        let session_id = acp::SessionId::new("test");
        let notification = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("test"),
            ))),
        );
        let result = handle.send_session_notification(notification);
        assert!(matches!(result, Err(AcpServerError::ConnectionUnavailable)));
    }

    #[test]
    fn typed_notification_on_unattached_handle_fails() {
        let handle = AcpConnectionHandle::new_disconnected();
        let result = handle.send_notification(ContextClearedParams::default());
        assert!(matches!(result, Err(AcpServerError::ConnectionUnavailable)));
    }

    #[tokio::test]
    async fn request_permission_on_unattached_handle_fails() {
        let handle = AcpConnectionHandle::new_disconnected();
        let request = RequestPermissionRequest::new(
            acp::SessionId::new("test"),
            acp::ToolCallUpdate::new(acp::ToolCallId::new("tool_1"), acp::ToolCallUpdateFields::new()),
            vec![acp::PermissionOption::new(
                acp::PermissionOptionId::new("allow-once"),
                "Allow once",
                acp::PermissionOptionKind::AllowOnce,
            )],
        );
        let result = handle.request_permission(request).await;
        assert!(matches!(result, Err(AcpServerError::ConnectionUnavailable)));
    }
}
