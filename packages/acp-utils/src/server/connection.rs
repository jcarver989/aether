use agent_client_protocol::schema::{RequestPermissionRequest, RequestPermissionResponse, SessionNotification};
use futures::future::BoxFuture;

use crate::notifications::{
    AuthMethodsUpdatedParams, ContextClearedParams, ContextUsageParams, ElicitationParams, ElicitationResponse,
    McpNotification, SubAgentProgressParams,
};

use super::{AcpConnectionHandle, AcpServerError};

/// Seam for outbound ACP server-side traffic. Production uses
/// [`AcpConnectionHandle`]; tests substitute an in-memory fake.
///
/// Methods correspond 1:1 to the typed notifications/requests Aether sends to
/// clients. New wire methods should be added here as fresh trait methods
/// rather than reintroducing a generic `send_ext_notification` — that
/// regression is what this design is meant to prevent.
pub trait AcpConnection: Send + Sync {
    fn send_session_notification(&self, notification: SessionNotification) -> Result<(), AcpServerError>;

    fn send_context_usage(&self, params: ContextUsageParams) -> Result<(), AcpServerError>;

    fn send_context_cleared(&self, params: ContextClearedParams) -> Result<(), AcpServerError>;

    fn send_auth_methods_updated(&self, params: AuthMethodsUpdatedParams) -> Result<(), AcpServerError>;

    fn send_sub_agent_progress(&self, params: SubAgentProgressParams) -> Result<(), AcpServerError>;

    fn send_mcp_notification(&self, notification: McpNotification) -> Result<(), AcpServerError>;

    fn request_elicitation(
        &self,
        params: ElicitationParams,
    ) -> BoxFuture<'_, Result<ElicitationResponse, AcpServerError>>;

    fn request_permission(
        &self,
        request: RequestPermissionRequest,
    ) -> BoxFuture<'_, Result<RequestPermissionResponse, AcpServerError>>;
}

impl AcpConnection for AcpConnectionHandle {
    fn send_session_notification(&self, notification: SessionNotification) -> Result<(), AcpServerError> {
        Self::send_session_notification(self, notification)
    }

    fn send_context_usage(&self, params: ContextUsageParams) -> Result<(), AcpServerError> {
        self.send_notification(params)
    }

    fn send_context_cleared(&self, params: ContextClearedParams) -> Result<(), AcpServerError> {
        self.send_notification(params)
    }

    fn send_auth_methods_updated(&self, params: AuthMethodsUpdatedParams) -> Result<(), AcpServerError> {
        self.send_notification(params)
    }

    fn send_sub_agent_progress(&self, params: SubAgentProgressParams) -> Result<(), AcpServerError> {
        self.send_notification(params)
    }

    fn send_mcp_notification(&self, notification: McpNotification) -> Result<(), AcpServerError> {
        self.send_notification(notification)
    }

    fn request_elicitation(
        &self,
        params: ElicitationParams,
    ) -> BoxFuture<'_, Result<ElicitationResponse, AcpServerError>> {
        Box::pin(self.send_request(params))
    }

    fn request_permission(
        &self,
        request: RequestPermissionRequest,
    ) -> BoxFuture<'_, Result<RequestPermissionResponse, AcpServerError>> {
        Box::pin(Self::request_permission(self, request))
    }
}
